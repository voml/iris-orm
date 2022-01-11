//! Native YYDB connector (Phase 2).
//!
//! Speaks YYDB's native VOS execution surface (`Connection::query` / sessions).
//! Never routes through `iris-adapter-*` foreign-store adapters.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod convert;
mod error;

use std::path::{Path, PathBuf};

use iris_ir::IrVersion;
use iris_types::{CapabilitySet, QueryCaps, WriteCaps};
use yydb::{Connection, PreparedPlan, SchemaVersion};

pub use error::{Error, Result};

/// Connector identifier.
pub const BACKEND_ID: &str = "yydb";

/// Schema handshake snapshot after open / ensure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaHandshake {
    /// Connector id (always [`BACKEND_ID`]).
    pub backend_id: &'static str,
    /// Stored schema version when present.
    pub schema_version: Option<u32>,
    /// YYDB DDL revision (session / prepared staleness).
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

    /// Read schema + revision handshake data.
    pub fn schema_handshake(&self) -> Result<SchemaHandshake> {
        let schema: Option<SchemaVersion> = self.conn.schema()?;
        let has_document = self.conn.parsed_schema()?.is_some();
        Ok(SchemaHandshake {
            backend_id: BACKEND_ID,
            schema_version: schema.map(|s| s.version),
            ddl_revision: self.conn.ddl_revision()?,
            has_document,
        })
    }

    /// Execute a VOS operation program on the native YYDB executor.
    pub fn execute_vos(&self, program: &str) -> Result<Vec<iris_types::Row>> {
        let rows = self.conn.query(program)?;
        Ok(rows.into_iter().map(convert::from_yydb_row).collect())
    }

    /// Prepare a VOS program against the current DDL revision.
    pub fn prepare(&self, program: &str) -> Result<PreparedVos> {
        Ok(PreparedVos {
            inner: PreparedPlan::prepare(&self.conn, program)?,
        })
    }

    /// Begin a data transaction.
    pub fn begin(&self) -> Result<()> {
        self.conn.begin()?;
        Ok(())
    }

    /// Commit the open data transaction.
    pub fn commit(&self) -> Result<()> {
        self.conn.commit()?;
        Ok(())
    }

    /// Roll back the open data transaction.
    pub fn rollback(&self) -> Result<()> {
        self.conn.rollback()?;
        Ok(())
    }

    /// Whether a data transaction is open.
    pub fn in_transaction(&self) -> bool {
        self.conn.in_transaction()
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

/// Prepared VOS plan pinned to a DDL revision.
#[derive(Debug, Clone)]
pub struct PreparedVos {
    inner: PreparedPlan,
}

impl PreparedVos {
    /// DDL revision at prepare time.
    pub fn ddl_revision(&self) -> u64 {
        self.inner.ddl_revision()
    }

    /// Execute if the database DDL revision still matches.
    pub fn execute(&self, source: &YydbSource) -> Result<Vec<iris_types::Row>> {
        let rows = self.inner.execute(&source.conn)?;
        Ok(rows.into_iter().map(convert::from_yydb_row).collect())
    }
}
