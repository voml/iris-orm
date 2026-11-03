//! Shared stateful Iris session core for host bindings (N-API and `pyo3`).
//!
//! Both host surfaces must expose the same profiles, the same VOS execution
//! semantics and the same diagnostic codes. Keeping the session state machine
//! in one crate is what makes that true: the bindings only translate this
//! surface into JavaScript or Python values.
//!
//! Failure shapes are structured. Every fallible call returns
//! [`ErrorEnvelope`] (`code` + `message`); the code is never folded back into
//! a display string, because callers assert on the code layer.

#![deny(clippy::all)]

use std::path::Path;

use iris::{
    CapabilitySet, DatasourceKind, Iris, Planner, ReferenceStore, Row,
    project::{expand_endpoint, load_project, read_schema},
    resolve_path,
};
use iris_adapter_mysql::MysqlSource;
use iris_adapter_postgres::PostgresSource;
use iris_adapter_sqlite::SqliteSource;
use iris_connector_yydb::YydbSource;
use iris_wasm::{bind, wire::rows_to_json};
use serde_json::{Value as JsonValue, json};

pub mod operation;

/// The host session has been closed.
pub const CODE_SESSION_CLOSED: &str = "IRIS-SESSION-CLOSED";
/// A parameter payload could not be bound into the VOS source.
pub const CODE_PARAM_ENCODING: &str = "IRIS-PARAM-ENCODING";
/// The structured operation JSON payload is not a valid Iris operation.
pub const CODE_OPERATION_INVALID: &str = "IRIS-OPERATION-INVALID";
/// The bound backend (or the reference runtime) rejected the execution.
pub const CODE_EXEC_FAILED: &str = "IRIS-EXEC-FAILED";
/// The active profile has no transaction control.
pub const CODE_TRANSACTION_UNSUPPORTED: &str = "IRIS-TRANSACTION-UNSUPPORTED";
/// A DSN or argument has the wrong shape.
pub const CODE_INVALID_ARGUMENT: &str = "IRIS-INVALID-ARGUMENT";
/// A DSN could not open its adapter.
pub const CODE_CONNECTION_FAILED: &str = "IRIS-CONNECTION-FAILED";

/// Structured failure shared by every host binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorEnvelope {
    /// Stable machine-readable code.
    pub code: String,
    /// Human-readable explanation.
    pub message: String,
}

impl ErrorEnvelope {
    /// Build an envelope from a stable code and a display message.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into() }
    }

    /// JSON form used on the wire: `{"code": …, "message": …}`.
    pub fn to_json(&self) -> JsonValue {
        json!({ "code": self.code, "message": self.message })
    }
}

/// Result of one session operation on the host wire.
#[derive(Debug, Clone)]
pub struct ExecuteOutcome {
    pub ok: bool,
    pub rows: Vec<JsonValue>,
    pub error: Option<ErrorEnvelope>,
}

impl ExecuteOutcome {
    /// Successful query with the encoded row set.
    pub fn rows(rows: Vec<Row>) -> Self {
        Self { ok: true, rows: rows_to_json(rows), error: None }
    }

    /// Successful unit-valued call (`execute`, `begin`, `commit`, `close`).
    pub fn unit() -> Self {
        Self { ok: true, rows: Vec::new(), error: None }
    }

    /// Structured failure.
    pub fn failure(error: ErrorEnvelope) -> Self {
        Self { ok: false, rows: Vec::new(), error: Some(error) }
    }

    /// Full envelope JSON: `{"ok": …, "rows": […], "error": {…} | null}`.
    pub fn to_json(&self) -> String {
        json!({
            "ok": self.ok,
            "rows": self.rows,
            "error": self.error.as_ref().map(ErrorEnvelope::to_json),
        })
        .to_string()
    }

    /// Row array JSON (N-API `rowsJson`).
    pub fn rows_json(&self) -> String {
        JsonValue::Array(self.rows.clone()).to_string()
    }

    /// Display message of the failure, if any.
    pub fn error_message(&self) -> Option<String> {
        self.error.as_ref().map(|err| err.message.clone())
    }

    /// Stable code of the failure, if any.
    pub fn error_code(&self) -> Option<String> {
        self.error.as_ref().map(|err| err.code.clone())
    }
}

fn from_iris_error(err: iris_types::Error) -> ErrorEnvelope {
    match err {
        iris_types::Error::Diagnostic(diag) => ErrorEnvelope::new(diag.code, diag.message),
        iris_types::Error::Ir(inner) => ErrorEnvelope::new(CODE_EXEC_FAILED, inner.to_string()),
        iris_types::Error::Runtime(message) => ErrorEnvelope::new(CODE_EXEC_FAILED, message),
    }
}

fn from_yydb_error(err: iris_connector_yydb::Error) -> ErrorEnvelope {
    match err {
        iris_connector_yydb::Error::Yydb(inner) => ErrorEnvelope::new(inner.code().to_string(), inner.to_string()),
        iris_connector_yydb::Error::NotReady(report) => ErrorEnvelope::new(report.code, report.message),
        iris_connector_yydb::Error::Runtime(message) | iris_connector_yydb::Error::Policy(message) => {
            ErrorEnvelope::new(CODE_EXEC_FAILED, message)
        }
    }
}

fn from_sqlite_error(err: iris_adapter_sqlite::Error) -> ErrorEnvelope {
    ErrorEnvelope::new(CODE_EXEC_FAILED, err.to_string())
}

fn from_postgres_error(err: iris_adapter_postgres::Error) -> ErrorEnvelope {
    ErrorEnvelope::new(CODE_EXEC_FAILED, err.to_string())
}

fn from_mysql_error(err: iris_adapter_mysql::Error) -> ErrorEnvelope {
    ErrorEnvelope::new(CODE_EXEC_FAILED, err.to_string())
}

fn closed() -> ErrorEnvelope {
    ErrorEnvelope::new(CODE_SESSION_CLOSED, "session closed")
}

fn transaction_unsupported(profile: &str) -> ErrorEnvelope {
    ErrorEnvelope::new(CODE_TRANSACTION_UNSUPPORTED, format!("{profile} sessions have no transaction control"))
}

enum SessionStore {
    Memory(Iris),
    Yydb(YydbSource),
    Sqlite(SqliteSource),
    Postgres(PostgresSource),
    Mysql(MysqlSource),
}

impl SessionStore {
    fn capabilities(&self) -> CapabilitySet {
        match self {
            Self::Memory(_) => CapabilitySet::reference_full(),
            Self::Yydb(_) => YydbSource::capabilities(),
            Self::Sqlite(_) => SqliteSource::capabilities(),
            Self::Postgres(_) => PostgresSource::capabilities(),
            Self::Mysql(_) => MysqlSource::capabilities(),
        }
    }

    fn query(&self, planner: &Planner, source: &str) -> Result<Vec<Row>, ErrorEnvelope> {
        match self {
            Self::Memory(iris) => iris.session().query(source).map_err(from_iris_error),
            Self::Yydb(db) => db.query(source).map_err(from_yydb_error),
            Self::Sqlite(db) => {
                let plan = planner.plan_source(source).map_err(from_iris_error)?;
                db.execute_plan(&plan).map_err(from_sqlite_error)
            }
            Self::Postgres(db) => {
                let plan = planner.plan_source(source).map_err(from_iris_error)?;
                db.execute_plan(&plan).map_err(from_postgres_error)
            }
            Self::Mysql(db) => {
                let plan = planner.plan_source(source).map_err(from_iris_error)?;
                db.execute_plan(&plan).map_err(from_mysql_error)
            }
        }
    }

    fn execute_unit(&self, planner: &Planner, source: &str) -> Result<(), ErrorEnvelope> {
        match self {
            Self::Memory(iris) => iris.session().execute(source).map_err(from_iris_error),
            Self::Yydb(db) => db.execute(source).map_err(from_yydb_error),
            Self::Sqlite(db) => {
                let plan = planner.plan_source(source).map_err(from_iris_error)?;
                db.execute_plan(&plan).map_err(from_sqlite_error)?;
                Ok(())
            }
            Self::Postgres(db) => {
                let plan = planner.plan_source(source).map_err(from_iris_error)?;
                db.execute_plan(&plan).map_err(from_postgres_error)?;
                Ok(())
            }
            Self::Mysql(db) => {
                let plan = planner.plan_source(source).map_err(from_iris_error)?;
                db.execute_plan(&plan).map_err(from_mysql_error)?;
                Ok(())
            }
        }
    }

    fn begin(&self) -> Result<(), ErrorEnvelope> {
        match self {
            Self::Memory(_) => Err(transaction_unsupported("memory")),
            Self::Yydb(db) => db.begin().map_err(from_yydb_error),
            Self::Sqlite(db) => db.begin().map_err(from_sqlite_error),
            Self::Postgres(_) => Err(transaction_unsupported("postgres")),
            Self::Mysql(_) => Err(transaction_unsupported("mysql")),
        }
    }

    fn commit(&self) -> Result<(), ErrorEnvelope> {
        match self {
            Self::Memory(_) => Err(transaction_unsupported("memory")),
            Self::Yydb(db) => db.commit().map_err(from_yydb_error),
            Self::Sqlite(db) => db.commit().map_err(from_sqlite_error),
            Self::Postgres(_) => Err(transaction_unsupported("postgres")),
            Self::Mysql(_) => Err(transaction_unsupported("mysql")),
        }
    }

    fn rollback(&self) -> Result<(), ErrorEnvelope> {
        match self {
            Self::Memory(_) => Err(transaction_unsupported("memory")),
            Self::Yydb(db) => db.rollback().map_err(from_yydb_error),
            Self::Sqlite(db) => db.rollback().map_err(from_sqlite_error),
            Self::Postgres(_) => Err(transaction_unsupported("postgres")),
            Self::Mysql(_) => Err(transaction_unsupported("mysql")),
        }
    }
}

/// Iris session bound to a reference or foreign adapter.
pub struct HostSession {
    store: Option<SessionStore>,
    planner: Planner,
    closed: bool,
    session_ddl_revision: Option<u64>,
}

fn open_yydb_endpoint(project_dir: &Path, endpoint: &str) -> Result<YydbSource, ErrorEnvelope> {
    if endpoint == ":memory:" {
        return YydbSource::open_in_memory().map_err(from_yydb_error);
    }
    let path = endpoint.strip_prefix("file:").unwrap_or(endpoint);
    let path = resolve_path(project_dir, path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| ErrorEnvelope::new(CODE_EXEC_FAILED, err.to_string()))?;
    }
    YydbSource::open(path).map_err(from_yydb_error)
}

impl HostSession {
    fn new_store(store: SessionStore, session_ddl_revision: Option<u64>) -> Self {
        let planner = Planner::new(store.capabilities());
        Self { store: Some(store), planner, closed: false, session_ddl_revision }
    }

    /// Reference profile: read-only, unseeded.
    pub fn open_memory() -> Self {
        Self::new_store(SessionStore::Memory(Iris::new(CapabilitySet::reference_full(), ReferenceStore::new())), None)
    }

    /// SQLite profile (`:memory:` or a file path).
    pub fn open_sqlite(path: &str) -> Result<Self, ErrorEnvelope> {
        let db = if path == ":memory:" { SqliteSource::open_in_memory() } else { SqliteSource::open(path) }.map_err(from_sqlite_error)?;
        Ok(Self::new_store(SessionStore::Sqlite(db), None))
    }

    /// YYDB profile over a `.yydb` file or `.yydx` directory layout.
    pub fn open_yydb(endpoint: &str) -> Result<Self, ErrorEnvelope> {
        let db = open_yydb_endpoint(Path::new("."), endpoint)?;
        Ok(Self::new_store(SessionStore::Yydb(db), None))
    }

    /// PostgreSQL profile over a real server URL.
    pub fn open_postgres(url: &str) -> Result<Self, ErrorEnvelope> {
        let db = PostgresSource::connect(url).map_err(from_postgres_error)?;
        Ok(Self::new_store(SessionStore::Postgres(db), None))
    }

    /// MySQL profile over a real server URL.
    pub fn open_mysql(url: &str) -> Result<Self, ErrorEnvelope> {
        let db = MysqlSource::connect(url).map_err(from_mysql_error)?;
        Ok(Self::new_store(SessionStore::Mysql(db), None))
    }

    /// Project profile resolved from an Iris config and datasource name.
    pub fn open_project(config_path: &str, source: &str) -> Result<Self, ErrorEnvelope> {
        let (project_dir, project) = load_project(Path::new(config_path)).map_err(|err| ErrorEnvelope::new(CODE_EXEC_FAILED, err))?;
        let ds = project.datasource(source).map_err(|err| ErrorEnvelope::new(CODE_EXEC_FAILED, err.to_string()))?;
        let (store, session_ddl_revision) = match ds.kind {
            DatasourceKind::Yydb => {
                let endpoint = expand_endpoint(ds, source).map_err(|err| ErrorEnvelope::new(CODE_EXEC_FAILED, err))?;
                let db = open_yydb_endpoint(&project_dir, &endpoint)?;
                let schema = read_schema(&project_dir, &project).map_err(|err| ErrorEnvelope::new(CODE_EXEC_FAILED, err))?;
                db.ensure_schema(&schema).map_err(from_yydb_error)?;
                let revision = db.schema_handshake().map_err(from_yydb_error)?.ddl_revision;
                (SessionStore::Yydb(db), Some(revision))
            }
            DatasourceKind::Sqlite => {
                let endpoint = expand_endpoint(ds, source).map_err(|err| ErrorEnvelope::new(CODE_EXEC_FAILED, err))?;
                let path = resolve_path(&project_dir, &endpoint);
                (SessionStore::Sqlite(SqliteSource::open(path).map_err(from_sqlite_error)?), None)
            }
            DatasourceKind::Postgres => {
                let url = expand_endpoint(ds, source).map_err(|err| ErrorEnvelope::new(CODE_EXEC_FAILED, err))?;
                (SessionStore::Postgres(PostgresSource::connect(&url).map_err(from_postgres_error)?), None)
            }
            DatasourceKind::Mysql => {
                let url = expand_endpoint(ds, source).map_err(|err| ErrorEnvelope::new(CODE_EXEC_FAILED, err))?;
                (SessionStore::Mysql(MysqlSource::connect(&url).map_err(from_mysql_error)?), None)
            }
            other => {
                return Err(ErrorEnvelope::new(
                    CODE_EXEC_FAILED,
                    format!("open_project_session does not support {:?} yet (use yydb/sqlite/postgres/mysql)", other),
                ));
            }
        };
        Ok(Self::new_store(store, session_ddl_revision))
    }

    /// Session selected by a DSN, shared by every host binding.
    ///
    /// This is the single DSN parser: generated clients pass their DSN here
    /// and the adapter never leaks into an authoring signature. Managed
    /// adapters (`sqlite:` and `file:`) apply `schema` when it is supplied,
    /// foreign servers get their DDL from migrations instead.
    pub fn open_dsn(dsn: &str, schema: Option<&str>) -> Result<Self, ErrorEnvelope> {
        let text = dsn.trim();
        if text.is_empty() {
            return Err(ErrorEnvelope::new(CODE_INVALID_ARGUMENT, "`dsn` must be a non-empty string"));
        }
        let (session, managed) = if text == "memory:" || text == "memory" {
            (Self::open_memory(), false)
        }
        else if let Some(path) = text.strip_prefix("sqlite:") {
            (Self::open_sqlite(if path.is_empty() { ":memory:" } else { path })?, true)
        }
        else if let Some(path) = text.strip_prefix("file:") {
            if path.is_empty() {
                return Err(ErrorEnvelope::new(CODE_INVALID_ARGUMENT, "`file:` DSN requires a path"));
            }
            (Self::open_yydb(path)?, true)
        }
        else if text.starts_with("mysql://") {
            (Self::open_mysql(text)?, false)
        }
        else if text.starts_with("postgres://") || text.starts_with("postgresql://") {
            (Self::open_postgres(text)?, false)
        }
        else {
            return Err(ErrorEnvelope::new(CODE_INVALID_ARGUMENT, format!("unrecognized DSN `{text}`")));
        };
        if managed {
            if let Some(schema) = schema {
                if !schema.trim().is_empty() {
                    session.managed_push(schema)?;
                }
            }
        }
        Ok(session)
    }

    /// Whether the session has been closed.
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    fn bind_parameters(&self, source: &str, parameters_json: Option<&str>) -> Result<bind::BoundParameters, ErrorEnvelope> {
        match parameters_json {
            Some(json) => bind::bind_parameters_typed(source, json).map_err(|err| ErrorEnvelope::new(CODE_PARAM_ENCODING, err)),
            None => Ok(bind::BoundParameters::unbound(source)),
        }
    }

    fn ensure_yydb_session_fresh(&self, db: &YydbSource) -> Result<(), ErrorEnvelope> {
        if let Some(expected) = self.session_ddl_revision {
            db.check_session_ddl_revision(expected).map_err(from_yydb_error)?;
        }
        Ok(())
    }

    /// Run a bound operation: YYDB takes typed values and classifies the
    /// source through its own frontend so writes reach the write executor,
    /// while other adapters take text-only reads through the Iris planner.
    fn run_bound_query(&self, bound: &bind::BoundParameters) -> ExecuteOutcome {
        let Some(store) = &self.store
        else {
            return ExecuteOutcome::failure(closed());
        };
        if let SessionStore::Yydb(db) = store {
            if let Err(err) = self.ensure_yydb_session_fresh(db) {
                return ExecuteOutcome::failure(err);
            }
            return match db.run_with_params(&bound.source, &bound.value_parameters) {
                Ok(rows) => ExecuteOutcome::rows(rows),
                Err(err) => ExecuteOutcome::failure(from_yydb_error(err)),
            };
        }
        let source = match bound.text_only() {
            Ok(source) => source,
            Err(err) => return ExecuteOutcome::failure(ErrorEnvelope::new(CODE_PARAM_ENCODING, err)),
        };
        match store.query(&self.planner, source) {
            Ok(rows) => ExecuteOutcome::rows(rows),
            Err(err) => ExecuteOutcome::failure(err),
        }
    }

    /// Plan + execute VOS DML (returns rows).
    pub fn query(&self, source: &str, parameters_json: Option<&str>) -> ExecuteOutcome {
        if self.closed {
            return ExecuteOutcome::failure(closed());
        }
        // Bind failures are request-level rejections: they travel the same
        // structured envelope as backend diagnostics instead of an untyped
        // host exception.
        let bound = match self.bind_parameters(source, parameters_json) {
            Ok(bound) => bound,
            Err(err) => return ExecuteOutcome::failure(err),
        };
        self.run_bound_query(&bound)
    }

    /// Execute unit-valued / DDL-shaped VOS.
    pub fn execute(&self, source: &str, parameters_json: Option<&str>) -> ExecuteOutcome {
        if self.closed {
            return ExecuteOutcome::failure(closed());
        }
        let bound = match self.bind_parameters(source, parameters_json) {
            Ok(bound) => bound,
            Err(err) => return ExecuteOutcome::failure(err),
        };
        let Some(store) = &self.store
        else {
            return ExecuteOutcome::failure(closed());
        };
        if let SessionStore::Yydb(db) = store {
            if let Err(err) = self.ensure_yydb_session_fresh(db) {
                return ExecuteOutcome::failure(err);
            }
            return match db.execute_with_params(&bound.source, &bound.value_parameters) {
                Ok(()) => ExecuteOutcome::unit(),
                Err(err) => ExecuteOutcome::failure(from_yydb_error(err)),
            };
        }
        let source = match bound.text_only() {
            Ok(source) => source,
            Err(err) => return ExecuteOutcome::failure(ErrorEnvelope::new(CODE_PARAM_ENCODING, err)),
        };
        match store.execute_unit(&self.planner, source) {
            Ok(()) => ExecuteOutcome::unit(),
            Err(err) => ExecuteOutcome::failure(err),
        }
    }

    /// Execute a structured Iris operation JSON payload (generated client ABI).
    pub fn execute_operation(&self, request_json: &str) -> ExecuteOutcome {
        if self.closed {
            return ExecuteOutcome::failure(closed());
        }
        let bound = match operation::encode_request_json(request_json) {
            Ok(bound) => bound,
            Err(err) => return ExecuteOutcome::failure(ErrorEnvelope::new(CODE_OPERATION_INVALID, err)),
        };
        self.run_bound_query(&bound)
    }

    /// Apply the managed-push schema to a YYDB or SQLite session.
    pub fn managed_push(&self, schema: &str) -> Result<(), ErrorEnvelope> {
        if self.closed {
            return Err(closed());
        }
        match &self.store {
            Some(SessionStore::Yydb(db)) => db.ensure_schema(schema).map_err(from_yydb_error).map(|_| ()),
            Some(SessionStore::Sqlite(db)) => db.managed_push(schema).map_err(from_sqlite_error).map(|_| ()),
            _ => Err(ErrorEnvelope::new(CODE_EXEC_FAILED, "managed_push is only supported on yydb and sqlite sessions")),
        }
    }

    /// Open a transaction on profiles that expose transaction control.
    pub fn begin(&self) -> ExecuteOutcome {
        if self.closed {
            return ExecuteOutcome::failure(closed());
        }
        match &self.store {
            Some(store) => match store.begin() {
                Ok(()) => ExecuteOutcome::unit(),
                Err(err) => ExecuteOutcome::failure(err),
            },
            None => ExecuteOutcome::failure(closed()),
        }
    }

    /// Commit the active transaction.
    pub fn commit(&self) -> ExecuteOutcome {
        if self.closed {
            return ExecuteOutcome::failure(closed());
        }
        match &self.store {
            Some(store) => match store.commit() {
                Ok(()) => ExecuteOutcome::unit(),
                Err(err) => ExecuteOutcome::failure(err),
            },
            None => ExecuteOutcome::failure(closed()),
        }
    }

    /// Roll back the active transaction.
    pub fn rollback(&self) -> ExecuteOutcome {
        if self.closed {
            return ExecuteOutcome::failure(closed());
        }
        match &self.store {
            Some(store) => match store.rollback() {
                Ok(()) => ExecuteOutcome::unit(),
                Err(err) => ExecuteOutcome::failure(err),
            },
            None => ExecuteOutcome::failure(closed()),
        }
    }

    /// Close the session, releasing the bound datasource.
    ///
    /// Dropping the store is what frees a file-backed datasource's writer
    /// lock, so `close` must do more than flip a flag: after it returns the
    /// same database file can be opened again from this process.
    pub fn close(&mut self) {
        self.closed = true;
        drop(self.store.take());
    }

    /// SQLite engine version for the active sqlite session.
    pub fn sqlite_engine_version(&self) -> Result<String, ErrorEnvelope> {
        if self.closed {
            return Err(closed());
        }
        match &self.store {
            Some(SessionStore::Sqlite(db)) => db.sqlite_version().map_err(from_sqlite_error),
            _ => Err(ErrorEnvelope::new(CODE_EXEC_FAILED, "sqlite identity requires a sqlite session")),
        }
    }

    /// SQLite engine source id for the active sqlite session.
    pub fn sqlite_source_id(&self) -> Result<String, ErrorEnvelope> {
        if self.closed {
            return Err(closed());
        }
        match &self.store {
            Some(SessionStore::Sqlite(db)) => db.sqlite_source_id().map_err(from_sqlite_error),
            _ => Err(ErrorEnvelope::new(CODE_EXEC_FAILED, "sqlite identity requires a sqlite session")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const USER_SCHEMA: &str = r#"
table User {
    @@user_id: utf8,
    @user_name: utf8,
    active: bool,
}
"#;

    fn temp_path(label: &str) -> std::path::PathBuf {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("iris-host-{label}-{stamp}"))
    }

    #[test]
    fn sqlite_profile_query_write_transaction_and_reopen() {
        let path = temp_path("sqlite").join("app.sqlite");
        let mut session = HostSession::open_sqlite(&path.to_string_lossy()).expect("open sqlite");
        session.managed_push(USER_SCHEMA).expect("push schema");

        let created = session.execute(r#"User::insert({ user_id: "u1", user_name: "alice", active: true })"#, None);
        assert!(created.ok, "{:?}", created.error);
        let rows = session.query(r#"User.filter(x => x.active).collect()"#, None);
        assert!(rows.ok, "{:?}", rows.error);
        assert_eq!(rows.rows.len(), 1);

        // Rolled-back writes leave the table untouched.
        assert!(session.begin().ok);
        assert!(session.execute(r#"User::insert({ user_id: "u2", user_name: "bob", active: false })"#, None).ok);
        assert!(session.rollback().ok);
        let rows = session.query("User.collect()", None);
        assert_eq!(rows.rows.len(), 1);

        session.close();
        let reopened = HostSession::open_sqlite(&path.to_string_lossy()).expect("reopen sqlite");
        let rows = reopened.query(r#"User.filter(x => x.user_id == "u1").collect()"#, None);
        assert!(rows.ok, "{:?}", rows.error);
        assert_eq!(rows.rows.len(), 1);
    }

    #[test]
    fn memory_profile_is_read_only_and_unseeded() {
        let session = HostSession::open_memory();
        let rows = session.query("User.collect()", None);
        assert!(!rows.ok);
        assert_eq!(rows.error_code().as_deref(), Some(CODE_EXEC_FAILED));

        let write = session.execute(r#"User::insert({ user_id: "u1", user_name: "alice", active: true })"#, None);
        assert!(!write.ok);
        assert_eq!(write.error_code().as_deref(), Some("IRIS-PLAN-REJECTED"));

        let txn = session.begin();
        assert!(!txn.ok);
        assert_eq!(txn.error_code().as_deref(), Some(CODE_TRANSACTION_UNSUPPORTED));
    }

    #[test]
    fn bind_and_operation_errors_keep_codes() {
        let session = HostSession::open_memory();
        let bound = session.query("User.filter(x => x.user_id == $id).collect()", Some("{not json"));
        assert!(!bound.ok);
        assert_eq!(bound.error_code().as_deref(), Some(CODE_PARAM_ENCODING));

        let operation = session.execute_operation("{");
        assert!(!operation.ok);
        assert_eq!(operation.error_code().as_deref(), Some(CODE_OPERATION_INVALID));
    }

    #[test]
    fn closed_session_reports_structured_code() {
        let mut session = HostSession::open_sqlite(":memory:").expect("open sqlite");
        session.close();
        let outcome = session.query("User.collect()", None);
        assert!(!outcome.ok);
        assert_eq!(outcome.error_code().as_deref(), Some(CODE_SESSION_CLOSED));
    }
}
