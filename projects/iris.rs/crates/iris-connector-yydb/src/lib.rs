//! Native YYDB connector.
//!
//! Iris speaks formal VOS / shared VOS IR to YYDB via the public `yydb::Connection`
//! facade. YYDB exposes the full VOS executor (prepare, query_prepared, query_vos,
//! execute_vos_write, transactions, schema handshake, doctor) on its Connection —
//! this connector wraps each capability with Iris-level policy and row mapping.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod error;

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use iris_ir::IrVersion;
use iris_types::{CapabilitySet, QueryCaps, WriteCaps};
use serde::{Deserialize, Serialize};
use yydb::{Connection, SchemaVersion, query::PreparedStatement};

pub use error::{Error, Result};

use yydb::Value as YydbValue;

// ---------------------------------------------------------------------------
// Readiness probe
// ---------------------------------------------------------------------------

/// Connector identifier.
pub const BACKEND_ID: &str = "yydb";

/// Stable diagnostic code when the native executor is unavailable.
pub const READINESS_CODE: &str = "IRIS-YYDB-VOS-EXECUTOR-NOT-READY";

/// Stable code when a prepared plan was built against an older DDL revision.
pub const PREPARED_STALE_CODE: &str = "VOS-PREPARED-STALE";

/// Stable code when a session was opened against an older DDL revision.
pub const SESSION_STALE_CODE: &str = "VOS-SESSION-STALE";

/// What Iris requires from YYDB before enabling native VOS execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessReport {
    /// Connector id.
    pub backend_id: String,
    /// True when schema install / read works on the public facade.
    pub schema_handshake_ready: bool,
    /// True when `Connection` exposes the VOS executor family (`prepare_vos`,
    /// `query_prepared`, `query_vos`, `execute_vos_write`) — live since the
    /// aligned `yydb` workspace.
    pub vos_executor_ready: bool,
    /// Stable diagnostic code.
    pub code: String,
    /// Human-readable blocker summary (no secrets).
    pub message: String,
}

impl ReadinessReport {
    /// Probe current readiness against the pinned `yydb` dependency.
    pub fn probe() -> Self {
        let schema_handshake_ready = Connection::open_in_memory().is_ok();
        // YYDB Connection now exposes the VOS executor family:
        //   prepare_vos → query_prepared / query_vos_with_params / query_vos
        //   execute_vos_write
        // plus transaction begin/commit/rollback and schema handshake.
        let vos_executor_ready = schema_handshake_ready;
        Self {
            backend_id: BACKEND_ID.into(),
            schema_handshake_ready,
            vos_executor_ready,
            code: if vos_executor_ready { "IRIS-YYDB-VOS-EXECUTOR-READY" } else { READINESS_CODE }.into(),
            message: if vos_executor_ready {
                "YYDB native VOS executor is live on the public Connection facade".into()
            }
            else {
                "YYDB native VOS executor not yet available on the Connection facade".into()
            },
        }
    }

    /// True only when every readiness bit is set.
    pub fn is_ready(&self) -> bool {
        self.schema_handshake_ready && self.vos_executor_ready
    }
}

// ---------------------------------------------------------------------------
// Schema handshake types
// ---------------------------------------------------------------------------

/// Schema handshake snapshot after open / ensure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaHandshake {
    /// Connector id (always [`BACKEND_ID`]).
    pub backend_id: &'static str,
    /// Stored schema version when present.
    pub schema_version: Option<u32>,
    /// Placeholder until YYDB exposes DDL revision on the public facade.
    pub ddl_revision: u64,
    /// Whether a VOS document is installed.
    pub has_document: bool,
}

// ---------------------------------------------------------------------------
// YydbSource — native datasource handle
// ---------------------------------------------------------------------------

/// Native YYDB datasource handle.
pub struct YydbSource {
    conn: Connection,
    path: Option<PathBuf>,
}

impl std::fmt::Debug for YydbSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("YydbSource").field("backend", &BACKEND_ID).field("path", &self.path).finish_non_exhaustive()
    }
}

impl YydbSource {
    /// Capability set advertised by this connector.
    pub fn capabilities() -> CapabilitySet {
        CapabilitySet {
            backend_id: BACKEND_ID.into(),
            backend_version: yydb::version().into(),
            ir_version_max: IrVersion::PHASE1,
            query: QueryCaps::full(),
            write: WriteCaps::full(),
            budget: iris_types::CompensationBudget::default(),
        }
    }

    /// Current readiness probe.
    pub fn readiness() -> ReadinessReport {
        ReadinessReport::probe()
    }

    fn require_vos_executor(&self) -> Result<()> {
        if !Self::readiness().is_ready() {
            return Err(Error::NotReady(Self::readiness()));
        }
        Ok(())
    }

    /// Open an in-memory YYDB (tests / ephemeral).
    pub fn open_in_memory() -> Result<Self> {
        Ok(Self { conn: Connection::open_in_memory()?, path: None })
    }

    /// Open or create a file-backed `.yydb`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        Ok(Self { conn: Connection::open(&path)?, path: Some(path) })
    }

    /// On-disk path when file-backed.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Borrow the underlying YYDB connection (escape hatch for advanced ops).
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Install / verify schema (Native Pull handshake input).
    pub fn ensure_schema(&self, document: &str) -> Result<()> {
        self.conn.ensure_schema(document)?;
        Ok(())
    }

    /// Read schema handshake data available on the current YYDB facade.
    pub fn schema_handshake(&self) -> Result<SchemaHandshake> {
        let schema: Option<SchemaVersion> = self.conn.schema()?;
        let has_document = schema.is_some();
        Ok(SchemaHandshake {
            backend_id: BACKEND_ID,
            schema_version: schema.as_ref().map(|s| s.version),
            ddl_revision: schema.map(|s| u64::from(s.version)).unwrap_or(0),
            has_document,
        })
    }

    /// Execute a VOS read pipeline and return typed Iris rows.
    pub fn query(&self, program: &str) -> Result<Vec<iris_types::Row>> {
        self.require_vos_executor()?;
        let rows = self.conn.query_vos(program)?;
        Ok(rows.into_iter().map(row_from_yydb).collect())
    }

    /// Execute a VOS write program (schema macros / DML).
    pub fn execute(&self, program: &str) -> Result<()> {
        self.require_vos_executor()?;
        self.conn.execute_vos_write(program)?;
        Ok(())
    }

    /// Prepare a VOS program into a cached plan.
    ///
    /// The plan is bound to the current DDL revision — re-prepare when the
    /// schema changes.
    pub fn prepare(&self, program: &str) -> Result<PreparedVos> {
        self.require_vos_executor()?;
        let handshake = self.schema_handshake()?;
        let stmt = self.conn.prepare_vos(program)?;
        Ok(PreparedVos { program: program.to_owned(), ddl_revision: handshake.ddl_revision, stmt })
    }

    /// Begin a data transaction.
    pub fn begin(&self) -> Result<()> {
        self.require_vos_executor()?;
        self.conn.begin().map_err(Error::from)
    }

    /// Commit the open data transaction.
    pub fn commit(&self) -> Result<()> {
        self.require_vos_executor()?;
        self.conn.commit().map_err(Error::from)
    }

    /// Roll back the open data transaction.
    pub fn rollback(&self) -> Result<()> {
        self.require_vos_executor()?;
        self.conn.rollback().map_err(Error::from)
    }

    /// Whether a data transaction is open.
    pub fn in_transaction(&self) -> bool {
        self.conn.in_transaction()
    }

    /// Reject use when the database DDL revision drifted since session open.
    pub fn check_session_ddl_revision(&self, expected: u64) -> Result<()> {
        self.require_vos_executor()?;
        let handshake = self.schema_handshake()?;
        if handshake.ddl_revision != expected {
            return Err(Error::Policy(format!(
                "{SESSION_STALE_CODE}: session opened at ddl revision {} but database is at {}",
                expected, handshake.ddl_revision
            )));
        }
        Ok(())
    }

    /// Re-open the same file path (drop + open). In-memory sources error.
    pub fn reopen(self) -> Result<Self> {
        let Some(path) = self.path.clone()
        else {
            return Err(Error::Runtime("in-memory YYDB cannot reopen by path".into()));
        };
        drop(self);
        Self::open(path)
    }
}

// ---------------------------------------------------------------------------
// PreparedVos — cached VOS query plan
// ---------------------------------------------------------------------------

/// Prepared VOS plan pinned to a DDL revision.
#[derive(Debug)]
pub struct PreparedVos {
    #[allow(dead_code)]
    program: String,
    ddl_revision: u64,
    stmt: PreparedStatement,
}

impl PreparedVos {
    /// DDL revision at prepare time.
    pub fn ddl_revision(&self) -> u64 {
        self.ddl_revision
    }

    /// The underlying YYDB prepared statement — escape hatch for advanced use.
    pub fn statement(&self) -> &PreparedStatement {
        &self.stmt
    }

    /// Execute with no named parameters (read-only projection).
    pub fn execute(&self, source: &YydbSource) -> Result<Vec<iris_types::Row>> {
        self.execute_with_params(source, BTreeMap::new())
    }

    /// Execute with named parameter bindings.
    pub fn execute_with_params(&self, source: &YydbSource, named: BTreeMap<String, yydb::Value>) -> Result<Vec<iris_types::Row>> {
        source.require_vos_executor()?;
        let handshake = source.schema_handshake()?;
        if handshake.ddl_revision != self.ddl_revision {
            return Err(Error::Policy(format!(
                "{PREPARED_STALE_CODE}: prepared at ddl revision {} but database is at {}",
                self.ddl_revision, handshake.ddl_revision
            )));
        }
        let rows = source.connection().query_prepared(&self.stmt, &named)?;
        Ok(rows.into_iter().map(row_from_yydb).collect())
    }
}

// ---------------------------------------------------------------------------
// Row / value conversion — yydb types → iris_types
// ---------------------------------------------------------------------------

/// Convert a YYDB runtime Value to Iris Value.
///
/// `bytes` keeps its octets: YYDB `Value::Bytes` becomes Iris
/// `Value::Bytes`, never text and never `Null`. The mapping is lossy for
/// variants Iris doesn't model (Vector, Object, File) — they collapse to Iris
/// `Null`. Callers that need full fidelity should work directly with the YYDB
/// `yydb::Value` via the `connection()` escape hatch.
pub fn value_from_yydb(v: YydbValue) -> iris_types::Value {
    match v {
        YydbValue::Null => iris_types::Value::Null,
        YydbValue::Bool(b) => iris_types::Value::Bool(b),
        YydbValue::Text(s) => iris_types::Value::Str(s),
        YydbValue::I64(i) => iris_types::Value::Int(i),
        YydbValue::U64(u) => iris_types::Value::Int(u as i64),
        YydbValue::F64(f) => iris_types::Value::Str(f.to_string()),
        YydbValue::Bytes(octets) => iris_types::Value::Bytes(octets),
        YydbValue::Uuid(u) => iris_types::Value::Str(u.to_string()),
        YydbValue::Vector(_) => iris_types::Value::Null,
        YydbValue::Object(_) => iris_types::Value::Null,
        YydbValue::File(_) => iris_types::Value::Null,
        YydbValue::Row(nested) => {
            let mut out = BTreeMap::new();
            for (k, v) in nested {
                out.insert(k, value_from_yydb(v));
            }
            iris_types::Value::Object(out)
        }
        _ => iris_types::Value::Null,
    }
}

/// Convert a YYDB query row to an Iris row (BTreeMap<String, Value> → BTreeMap<String, iris_types::Value>).
pub fn row_from_yydb(row: BTreeMap<String, YydbValue>) -> iris_types::Row {
    let mut out = BTreeMap::new();
    for (k, v) in row {
        out.insert(k, value_from_yydb(v));
    }
    out
}

// ---------------------------------------------------------------------------
// Deprecated shims — kept for backwards compat
// ---------------------------------------------------------------------------

impl PreparedVos {
    /// Execute if the database DDL revision still matches.
    #[deprecated(note = "renamed to PreparedVos::execute")]
    pub fn execute_stale_checked(&self, source: &YydbSource) -> Result<Vec<iris_types::Row>> {
        self.execute(source)
    }

    /// Execute with named parameters, stale-checked.
    #[deprecated(note = "renamed to PreparedVos::execute_with_params")]
    pub fn execute_params_stale_checked(&self, source: &YydbSource, named: BTreeMap<String, yydb::Value>) -> Result<Vec<iris_types::Row>> {
        self.execute_with_params(source, named)
    }
}

impl YydbSource {
    /// Execute a VOS operation program on the native YYDB executor.
    #[deprecated(note = "renamed to YydbSource::query (aligns with Session::query / db.$query)")]
    pub fn execute_vos(&self, program: &str) -> Result<Vec<iris_types::Row>> {
        self.query(program)
    }
}

// Silence unused imports when deprecated shims compile.
#[allow(dead_code)]
fn _unreferenced() {
    let _ = BTreeSet::<()>::new();
}
