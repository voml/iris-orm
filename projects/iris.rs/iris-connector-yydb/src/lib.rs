//! Native YYDB connector (Phase 2 readiness gate).
//!
//! Iris will speak **formal VOS / shared VOS IR** to YYDB only. Until YYDB's
//! public `Connection` exposes a versioned VOS executor (`query`, sessions,
//! prepared plans), this crate wires schema handshake only and refuses DML/query.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod error;

use std::path::{Path, PathBuf};

use iris_ir::IrVersion;
use iris_types::{CapabilitySet, QueryCaps, WriteCaps};
use serde::{Deserialize, Serialize};
use yydb::{Connection, SchemaVersion};

pub use error::{Error, Result};

/// Connector identifier.
pub const BACKEND_ID: &str = "yydb";

/// Stable readiness code for tooling and diagnostics.
pub const READINESS_CODE: &str = "IRIS-YYDB-VOS-EXECUTOR-NOT-READY";

/// What Iris requires from YYDB before enabling native VOS execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessReport {
    /// Connector id.
    pub backend_id: String,
    /// True when schema install / read works on the public facade.
    pub schema_handshake_ready: bool,
    /// False until YYDB publishes VOS `query` + session APIs on `Connection`.
    pub vos_executor_ready: bool,
    /// Stable diagnostic code.
    pub code: String,
    /// Human-readable blocker summary (no secrets).
    pub message: String,
}

impl ReadinessReport {
    /// Probe current readiness against the pinned `yydb` git dependency.
    pub fn probe() -> Self {
        let schema_handshake_ready = Connection::open_in_memory().is_ok();
        let vos_executor_ready = false;
        Self {
            backend_id: BACKEND_ID.into(),
            schema_handshake_ready,
            vos_executor_ready,
            code: READINESS_CODE.into(),
            message: "YYDB formal VOS executor (query / sessions / prepared plans) is not yet \
                      exported on the public Connection facade; Iris refuses DML/query until then"
                .into(),
        }
    }

    /// True only when every readiness bit is set.
    pub fn is_ready(&self) -> bool {
        self.schema_handshake_ready && self.vos_executor_ready
    }
}

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

/// Native YYDB datasource handle.
pub struct YydbSource {
    conn: Connection,
    path: Option<PathBuf>,
}

impl std::fmt::Debug for YydbSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("YydbSource")
            .field("backend", &BACKEND_ID)
            .field("path", &self.path)
            .finish_non_exhaustive()
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
        Ok(Self {
            conn: Connection::open_in_memory()?,
            path: None,
        })
    }

    /// Open or create a file-backed `.yydb`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        Ok(Self {
            conn: Connection::open(&path)?,
            path: Some(path),
        })
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
    pub fn ensure_schema(&self, version: u32, document: &str) -> Result<()> {
        self.conn.ensure_schema(version, document)?;
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

    /// Execute a VOS operation program on the native YYDB executor.
    pub fn execute_vos(&self, _program: &str) -> Result<Vec<iris_types::Row>> {
        self.require_vos_executor()?;
        Err(Error::Policy(
            "readiness cleared but VOS client binding is not implemented yet".into(),
        ))
    }

    /// Prepare a VOS program against the current DDL revision.
    pub fn prepare(&self, _program: &str) -> Result<PreparedVos> {
        self.require_vos_executor()?;
        Err(Error::Policy(
            "readiness cleared but prepared VOS binding is not implemented yet".into(),
        ))
    }

    /// Begin a data transaction.
    pub fn begin(&self) -> Result<()> {
        self.require_vos_executor()
    }

    /// Commit the open data transaction.
    pub fn commit(&self) -> Result<()> {
        self.require_vos_executor()
    }

    /// Roll back the open data transaction.
    pub fn rollback(&self) -> Result<()> {
        self.require_vos_executor()
    }

    /// Whether a data transaction is open.
    pub fn in_transaction(&self) -> bool {
        false
    }

    /// Re-open the same file path (drop + open). In-memory sources error.
    pub fn reopen(self) -> Result<Self> {
        let Some(path) = self.path.clone() else {
            return Err(Error::Runtime(
                "in-memory YYDB cannot reopen by path".into(),
            ));
        };
        drop(self);
        Self::open(path)
    }
}

/// Prepared VOS plan pinned to a DDL revision (not yet wired).
#[derive(Debug, Clone)]
pub struct PreparedVos {
    ddl_revision: u64,
}

impl PreparedVos {
    /// DDL revision at prepare time.
    pub fn ddl_revision(&self) -> u64 {
        self.ddl_revision
    }

    /// Execute if the database DDL revision still matches.
    pub fn execute(&self, source: &YydbSource) -> Result<Vec<iris_types::Row>> {
        source.require_vos_executor()?;
        Err(Error::Policy(
            "readiness cleared but prepared execute binding is not implemented yet".into(),
        ))
    }
}
