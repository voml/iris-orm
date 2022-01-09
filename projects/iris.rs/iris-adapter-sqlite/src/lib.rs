//! Isolated SQLite foreign-store adapter.
//!
//! Backend commands stay **private** to this crate. The public API speaks Iris
//! physical plans, observed catalogs, and mapping manifests --?never SQL strings.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod catalog;
mod execute;
mod migrate;
mod outbox;
mod types;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use iris_ir::{CommitToken, IrVersion, OutboxRecord, PhysicalPlan};
use iris_types::{
    CapabilitySet, DriftReport, LogicalMigrationPlan, MappingManifest, ObservedCatalog, QueryCaps,
    Row, RowWrite, WriteCaps,
};
use rusqlite::Connection;

pub use catalog::adopt_plan;
pub use migrate::{PushReport, apply_push, plan_push};
pub use outbox::AuthorityTxn;

/// Adapter identifier.
pub const BACKEND_ID: &str = "sqlite";

/// Adapter crate version label.
pub const ADAPTER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// SQLite foreign datasource.
pub struct SqliteSource {
    conn: Mutex<Connection>,
    path: Option<PathBuf>,
}

impl std::fmt::Debug for SqliteSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteSource")
            .field("backend", &BACKEND_ID)
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl SqliteSource {
    /// Capability set for this adapter.
    pub fn capabilities() -> CapabilitySet {
        CapabilitySet {
            backend_id: BACKEND_ID.into(),
            backend_version: ADAPTER_VERSION.into(),
            ir_version_max: IrVersion::PHASE1,
            query: QueryCaps::full(),
            write: WriteCaps::full(),
            budget: iris_types::CompensationBudget::default(),
        }
    }

    /// Open an in-memory SQLite database.
    pub fn open_in_memory() -> Result<Self> {
        Ok(Self {
            conn: Mutex::new(Connection::open_in_memory()?),
            path: None,
        })
    }

    /// Open or create a file-backed SQLite database.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Self {
            conn: Mutex::new(Connection::open(&path)?),
            path: Some(path),
        })
    }

    /// On-disk path when file-backed.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Inspect the foreign catalog (Adopt Existing input).
    pub fn inspect(&self) -> Result<ObservedCatalog> {
        let conn = self.conn.lock().expect("sqlite mutex");
        catalog::inspect_catalog(&conn)
    }

    /// Build an adopt mapping against a VOS schema document (does not invent semantics).
    pub fn adopt(&self, vos_schema: &str) -> Result<MappingManifest> {
        let catalog = self.inspect()?;
        let document = vos::parser::parse_document(vos_schema).map_err(|d| {
            Error::Vos(format!(
                "parse schema: {}",
                d.errors
                    .first()
                    .map(|e| e.message.as_str())
                    .unwrap_or("unknown")
            ))
        })?;
        Ok(catalog::adopt_plan(&document, &catalog))
    }

    /// Plan a Managed Push from a VOS schema (reviewable logical plan).
    pub fn plan_managed_push(&self, vos_schema: &str) -> Result<LogicalMigrationPlan> {
        let document = vos::parser::parse_document(vos_schema).map_err(|d| {
            Error::Vos(format!(
                "parse schema: {}",
                d.errors
                    .first()
                    .map(|e| e.message.as_str())
                    .unwrap_or("unknown")
            ))
        })?;
        let observed = self.inspect()?;
        migrate::plan_push(&document, &observed)
    }

    /// Apply a previously reviewed Managed Push plan.
    pub fn apply_managed_push(
        &self,
        plan: &LogicalMigrationPlan,
        vos_schema: &str,
    ) -> Result<PushReport> {
        let document = vos::parser::parse_document(vos_schema).map_err(|d| {
            Error::Vos(format!(
                "parse schema: {}",
                d.errors
                    .first()
                    .map(|e| e.message.as_str())
                    .unwrap_or("unknown")
            ))
        })?;
        let mut conn = self.conn.lock().expect("sqlite mutex");
        migrate::apply_push(&mut conn, plan, &document)
    }

    /// Convenience: plan + apply when the database has no conflicting tables.
    pub fn managed_push(&self, vos_schema: &str) -> Result<PushReport> {
        let plan = self.plan_managed_push(vos_schema)?;
        if plan.destructive {
            return Err(Error::Policy(
                "destructive managed push requires explicit review/apply".into(),
            ));
        }
        self.apply_managed_push(&plan, vos_schema)
    }

    /// Drift check: local VOS vs observed catalog.
    pub fn drift(&self, vos_schema: &str) -> Result<DriftReport> {
        let document = vos::parser::parse_document(vos_schema).map_err(|d| {
            Error::Vos(format!(
                "parse schema: {}",
                d.errors
                    .first()
                    .map(|e| e.message.as_str())
                    .unwrap_or("unknown")
            ))
        })?;
        let catalog = self.inspect()?;
        Ok(catalog::drift_report(&document, &catalog))
    }

    /// Execute an Iris physical plan (reads).
    pub fn execute_plan(&self, plan: &PhysicalPlan) -> Result<Vec<Row>> {
        if plan.is_rejected() {
            return Err(Error::Policy(
                plan.rejection_note().unwrap_or("plan rejected").to_string(),
            ));
        }
        let conn = self.conn.lock().expect("sqlite mutex");
        execute::execute_plan(&conn, plan)
    }

    /// Insert a row (typed values; SQL stays private).
    pub fn insert(&self, write: &RowWrite) -> Result<()> {
        let conn = self.conn.lock().expect("sqlite mutex");
        execute::insert_row(&conn, write)
    }

    /// Update by primary key.
    pub fn update(&self, write: &RowWrite) -> Result<usize> {
        let conn = self.conn.lock().expect("sqlite mutex");
        execute::update_row(&conn, write)
    }

    /// Delete by primary key.
    pub fn delete(&self, table: &str, primary_key: &str, key: &iris_types::Value) -> Result<usize> {
        let conn = self.conn.lock().expect("sqlite mutex");
        execute::delete_row(&conn, table, primary_key, key)
    }

    /// Begin a transaction.
    pub fn begin(&self) -> Result<()> {
        let conn = self.conn.lock().expect("sqlite mutex");
        conn.execute_batch("BEGIN IMMEDIATE")?;
        Ok(())
    }

    /// Commit.
    pub fn commit(&self) -> Result<()> {
        let conn = self.conn.lock().expect("sqlite mutex");
        conn.execute_batch("COMMIT")?;
        Ok(())
    }

    /// Rollback.
    pub fn rollback(&self) -> Result<()> {
        let conn = self.conn.lock().expect("sqlite mutex");
        conn.execute_batch("ROLLBACK")?;
        Ok(())
    }

    /// Install authority commit counter + outbox tables (idempotent).
    pub fn ensure_authority_outbox(&self) -> Result<()> {
        let conn = self.conn.lock().expect("sqlite mutex");
        outbox::ensure_schema(&conn)
    }

    /// Current authority [`CommitToken`] (last successful composite commit).
    pub fn current_commit_token(&self) -> Result<CommitToken> {
        let conn = self.conn.lock().expect("sqlite mutex");
        outbox::current_token(&conn)
    }

    /// Authority transaction: business mutations + outbox appends commit atomically.
    ///
    /// Success means authority committed and durable outbox accepted propagation duty --?    /// not that cache/search/vector projections have applied the events.
    pub fn authority_commit<R>(
        &self,
        f: impl FnOnce(&mut AuthorityTxn<'_>) -> Result<R>,
    ) -> Result<(R, CommitToken)> {
        let mut conn = self.conn.lock().expect("sqlite mutex");
        outbox::authority_commit(&mut conn, f)
    }

    /// Poll durable outbox events after a sequence (projector shape).
    pub fn outbox_after(&self, after_seq: u64, limit: usize) -> Result<Vec<OutboxRecord>> {
        let conn = self.conn.lock().expect("sqlite mutex");
        outbox::outbox_after(&conn, after_seq, limit)
    }

    /// Outbox backlog size.
    pub fn outbox_backlog(&self) -> Result<u64> {
        let conn = self.conn.lock().expect("sqlite mutex");
        outbox::outbox_backlog(&conn)
    }
}

/// Adapter errors.
#[derive(Debug)]
pub enum Error {
    /// SQLite / rusqlite failure.
    Sqlite(rusqlite::Error),
    /// I/O (journal files, directories).
    Io(std::io::Error),
    /// VOS schema parse / semantic issues.
    Vos(String),
    /// Policy / adopt blocker / destructive without review.
    Policy(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlite(e) => write!(f, "{e}"),
            Self::Io(e) => write!(f, "{e}"),
            Self::Vos(s) | Self::Policy(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(e) => Some(e),
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(value: rusqlite::Error) -> Self {
        Self::Sqlite(value)
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

/// Result alias.
pub type Result<T> = std::result::Result<T, Error>;
