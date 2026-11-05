//! Python (`pyo3`) binding surface for Iris ORM.
//!
//! The binding is a thin wrapper over [`iris::host::HostSession`], the same
//! session core consumed by the Node N-API binding. Both surfaces expose the
//! same profiles (`memory`, `sqlite`, `yydb`, `postgres`, `mysql`), the same
//! VOS execution semantics and the same diagnostic codes. Session calls do
//! not parse `.iris` sources, do not accept SQL, and do not define a second
//! query DSL. Schema parsing for codegen lives in the facade's `generator`
//! module and is exposed as a separate `generate` primitive.
//!
//! Fallible session calls return a structured JSON envelope
//! (`{"ok": …, "rows": […], "error": {"code": …, "message": …} | null}`) so
//! callers can assert on the code instead of matching display text. Session
//! construction failures raise a `RuntimeError` whose message is the same
//! JSON envelope; the Python wrapper re-raises it as `IrisError` with `code`.

#![deny(clippy::all)]

use iris::host::{ErrorEnvelope, ExecuteOutcome, HostSession};
use pyo3::{exceptions::PyRuntimeError, prelude::*};
use serde_json::json;
use std::path::Path;

/// Capability ids exported by this binding.
const CAPABILITIES: [&str; 10] = [
    "version",
    "contract_version",
    "capabilities",
    "generate",
    "open_memory",
    "open_sqlite",
    "open_yydb",
    "open_postgres",
    "open_mysql",
    "open_dsn",
];

fn raise_open(err: ErrorEnvelope) -> PyErr {
    PyRuntimeError::new_err(err.to_json().to_string())
}

fn closed_envelope() -> String {
    ExecuteOutcome::failure(ErrorEnvelope::invalid_argument(iris::host::DIAG_SESSION_CLOSED, "session closed")).to_json()
}

/// Iris facade version (`iris::version()`).
#[pyfunction]
fn version() -> &'static str {
    iris::version()
}

/// Contract envelope format accepted by Iris (`iris::ACCEPTED_CONTRACT_FORMAT`).
#[pyfunction]
fn contract_version() -> &'static str {
    iris::ACCEPTED_CONTRACT_FORMAT
}

/// Read-only capability ids exposed by this binding.
#[pyfunction]
fn capabilities() -> Vec<&'static str> {
    CAPABILITIES.to_vec()
}

/// Session bound to a reference or foreign adapter.
#[pyclass(unsendable, name = "Session")]
pub struct Session {
    inner: Option<HostSession>,
}

impl Session {
    fn from_open(result: Result<HostSession, ErrorEnvelope>) -> PyResult<Self> {
        match result {
            Ok(inner) => Ok(Self { inner: Some(inner) }),
            Err(err) => Err(raise_open(err)),
        }
    }

    fn with_session<T>(&self, fallback: T, run: impl FnOnce(&HostSession) -> T) -> T {
        match &self.inner {
            Some(inner) => run(inner),
            None => fallback,
        }
    }
}

#[pymethods]
impl Session {
    /// Plan + execute VOS DML and return the encoded row set.
    fn query(&self, source: &str, parameters_json: Option<&str>) -> String {
        self.with_session(closed_envelope(), |session| session.query(source, parameters_json).to_json())
    }

    /// Execute unit-valued / DDL-shaped VOS.
    fn execute(&self, source: &str, parameters_json: Option<&str>) -> String {
        self.with_session(closed_envelope(), |session| session.execute(source, parameters_json).to_json())
    }

    /// Execute a structured Iris operation JSON payload (generated client ABI).
    #[pyo3(name = "execute_operation")]
    fn execute_operation(&self, request_json: &str) -> String {
        self.with_session(closed_envelope(), |session| session.execute_operation(request_json).to_json())
    }

    /// Apply the managed-push schema to a YYDB or SQLite session.
    #[pyo3(name = "managed_push")]
    fn managed_push(&self, schema: &str) -> String {
        self.with_session(closed_envelope(), |session| match session.managed_push(schema) {
            Ok(()) => ExecuteOutcome::unit().to_json(),
            Err(err) => ExecuteOutcome::failure(err).to_json(),
        })
    }

    /// Open a transaction on YYDB or SQLite sessions.
    fn begin(&self) -> String {
        self.with_session(closed_envelope(), |session| session.begin().to_json())
    }

    /// Commit the active transaction.
    fn commit(&self) -> String {
        self.with_session(closed_envelope(), |session| session.commit().to_json())
    }

    /// Roll back the active transaction.
    fn rollback(&self) -> String {
        self.with_session(closed_envelope(), |session| session.rollback().to_json())
    }

    /// SQLite engine version for the active sqlite session.
    #[pyo3(name = "sqlite_engine_version")]
    fn sqlite_engine_version(&self) -> PyResult<String> {
        match &self.inner {
            Some(session) => session.sqlite_engine_version().map_err(raise_open),
            None => Err(raise_open(ErrorEnvelope::invalid_argument(iris::host::DIAG_SESSION_CLOSED, "session closed"))),
        }
    }

    /// SQLite engine source id for the active sqlite session.
    #[pyo3(name = "sqlite_source_id")]
    fn sqlite_source_id(&self) -> PyResult<String> {
        match &self.inner {
            Some(session) => session.sqlite_source_id().map_err(raise_open),
            None => Err(raise_open(ErrorEnvelope::invalid_argument(iris::host::DIAG_SESSION_CLOSED, "session closed"))),
        }
    }

    /// Close the session, releasing the bound datasource.
    fn close(&mut self) {
        if let Some(mut inner) = self.inner.take() {
            inner.close();
        }
    }
}

/// Generate client bindings (`rust` | `typescript` | `python`) from merged schema source.
///
/// Parsing and emission stay in the facade (`iris::generator`); this function
/// only maps the outcome to the same JSON envelope the Node binding returns.
/// `datasource` is required for the `rust` target and ignored by the others.
#[pyfunction]
#[pyo3(signature = (source, target, out_dir, datasource = None))]
fn generate(source: &str, target: &str, out_dir: &str, datasource: Option<&str>) -> String {
    match iris::generator::generate_from_source(source, target, Path::new(out_dir), datasource) {
        Ok((model, paths)) => {
            let output_path = match target {
                "rust" => iris::generator::rust_target_dir(Path::new(out_dir)).display().to_string(),
                _ => out_dir.to_string(),
            };
            json!({
                "ok": true,
                "output_path": output_path,
                "schema_fingerprint": model.schema_fingerprint,
                "files": paths.iter().map(|path| path.display().to_string()).collect::<Vec<_>>(),
                "error": null,
            })
            .to_string()
        }
        Err(err) => json!({
            "ok": false,
            "output_path": "",
            "schema_fingerprint": "",
            "files": [],
            "error": err.to_string(),
        })
        .to_string(),
    }
}

/// Open the reference profile (read-only, unseeded).
#[pyfunction]
fn open_memory() -> Session {
    Session { inner: Some(HostSession::open_memory()) }
}

/// Open a SQLite profile (`:memory:` or a file path).
#[pyfunction]
#[pyo3(signature = (path = ":memory:"))]
fn open_sqlite(path: &str) -> PyResult<Session> {
    Session::from_open(HostSession::open_sqlite(path))
}

/// Open a YYDB profile over a `.yydb` file or `.yydx` directory layout.
#[pyfunction]
#[pyo3(signature = (path = ":memory:"))]
fn open_yydb(path: &str) -> PyResult<Session> {
    Session::from_open(HostSession::open_yydb(path))
}

/// Open a PostgreSQL profile over a real server URL.
#[pyfunction]
fn open_postgres(url: &str) -> PyResult<Session> {
    Session::from_open(HostSession::open_postgres(url))
}

/// Open a MySQL profile over a real server URL.
#[pyfunction]
fn open_mysql(url: &str) -> PyResult<Session> {
    Session::from_open(HostSession::open_mysql(url))
}

/// Open the adapter a DSN selects, applying `schema` on managed adapters.
///
/// This is the construction path behind generated `create_client(dsn)`: one
/// DSN parser in `iris_host`, no backend name in an authoring signature.
#[pyfunction]
#[pyo3(signature = (dsn, schema = None))]
fn open_dsn(dsn: &str, schema: Option<&str>) -> PyResult<Session> {
    Session::from_open(HostSession::open_dsn(dsn, schema))
}

/// Native module entry point (`import iris_pyo3`).
#[pymodule]
fn iris_pyo3(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(version, module)?)?;
    module.add_function(wrap_pyfunction!(contract_version, module)?)?;
    module.add_function(wrap_pyfunction!(capabilities, module)?)?;
    module.add_function(wrap_pyfunction!(generate, module)?)?;
    module.add_function(wrap_pyfunction!(open_memory, module)?)?;
    module.add_function(wrap_pyfunction!(open_sqlite, module)?)?;
    module.add_function(wrap_pyfunction!(open_yydb, module)?)?;
    module.add_function(wrap_pyfunction!(open_postgres, module)?)?;
    module.add_function(wrap_pyfunction!(open_mysql, module)?)?;
    module.add_function(wrap_pyfunction!(open_dsn, module)?)?;
    module.add_class::<Session>()?;
    Ok(())
}
