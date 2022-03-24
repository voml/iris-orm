//! Isolated PostgreSQL foreign-store adapter.
//!
//! Backend commands stay **private**. Catalog inspect, type mapping, and DDL
//! emission are PostgreSQL-specific --?not shared with MySQL/SQLite adapters.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod catalog;
mod execute;
mod migrate;
mod outbox;

use iris_ir::{CommitToken, IrVersion, OutboxRecord, PhysicalPlan};
use iris_types::{
    CapabilitySet, DriftReport, LogicalMigrationPlan, MappingManifest, ObservedCatalog, QueryCaps,
    Row, RowWrite, WriteCaps,
};
use r2d2::Pool;
use r2d2_postgres::{PostgresConnectionManager, postgres::NoTls};

pub use catalog::{adopt_plan, classify_type};
pub use migrate::{PushReport, apply_push, plan_push};
pub use outbox::AuthorityTxn;

/// Adapter identifier.
pub const BACKEND_ID: &str = "postgres";

/// Adapter crate version label.
pub const ADAPTER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// PostgreSQL foreign datasource backed by an r2d2 pool.
pub struct PostgresSource {
    pool: Pool<PostgresConnectionManager<NoTls>>,
}

impl std::fmt::Debug for PostgresSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostgresSource")
            .field("backend", &BACKEND_ID)
            .field("pool_state", &self.pool.state())
            .finish_non_exhaustive()
    }
}

impl PostgresSource {
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

    /// Connect using a `postgres` crate connection string / URL.
    ///
    /// Example: `host=127.0.0.1 user=iris password=iris dbname=iris`
    pub fn connect(conninfo: &str) -> Result<Self> {
        Self::connect_with_pool_timeout(conninfo, std::time::Duration::from_secs(5))
    }

    /// Connect with an explicit pool establish timeout (tests / tight SLAs).
    pub fn connect_with_pool_timeout(
        conninfo: &str,
        pool_timeout: std::time::Duration,
    ) -> Result<Self> {
        let config = conninfo
            .parse()
            .map_err(|e| Error::Config(format!("{e}")))?;
        let manager = PostgresConnectionManager::new(config, NoTls);
        let pool = Pool::builder()
            .max_size(8)
            .connection_timeout(pool_timeout)
            .build(manager)
            .map_err(|e| Error::Pool(e.to_string()))?;
        Ok(Self { pool })
    }

    /// Pool state (for connection-pool conformance checks).
    pub fn pool_connections(&self) -> u32 {
        self.pool.state().connections
    }

    /// Inspect the foreign catalog (Adopt Existing input).
    pub fn inspect(&self) -> Result<ObservedCatalog> {
        let mut client = self.pool.get().map_err(|e| Error::Pool(e.to_string()))?;
        catalog::inspect_catalog(&mut client)
    }

    /// Build an adopt mapping against a VOS schema document.
    pub fn adopt(&self, vos_schema: &str) -> Result<MappingManifest> {
        let catalog = self.inspect()?;
        let document = parse_vos(vos_schema)?;
        Ok(catalog::adopt_plan(&document, &catalog))
    }

    /// Plan a Managed Push from a VOS schema.
    pub fn plan_managed_push(&self, vos_schema: &str) -> Result<LogicalMigrationPlan> {
        let document = parse_vos(vos_schema)?;
        let observed = self.inspect()?;
        migrate::plan_push(&document, &observed)
    }

    /// Apply a previously reviewed Managed Push plan.
    pub fn apply_managed_push(
        &self,
        plan: &LogicalMigrationPlan,
        vos_schema: &str,
    ) -> Result<PushReport> {
        let document = parse_vos(vos_schema)?;
        let mut client = self.pool.get().map_err(|e| Error::Pool(e.to_string()))?;
        migrate::apply_push(&mut client, plan, &document)
    }

    /// Convenience: plan + apply when non-destructive.
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
        let document = parse_vos(vos_schema)?;
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
        let mut client = self.pool.get().map_err(|e| Error::Pool(e.to_string()))?;
        execute::execute_plan(&mut client, plan)
    }

    /// Insert a row.
    pub fn insert(&self, write: &RowWrite) -> Result<()> {
        let mut client = self.pool.get().map_err(|e| Error::Pool(e.to_string()))?;
        execute::insert_row(&mut *client, write)
    }

    /// Update by primary key.
    pub fn update(&self, write: &RowWrite) -> Result<u64> {
        let mut client = self.pool.get().map_err(|e| Error::Pool(e.to_string()))?;
        execute::update_row(&mut *client, write)
    }

    /// Delete by primary key.
    pub fn delete(&self, table: &str, primary_key: &str, key: &iris_types::Value) -> Result<u64> {
        let mut client = self.pool.get().map_err(|e| Error::Pool(e.to_string()))?;
        execute::delete_row(&mut *client, table, primary_key, key)
    }

    /// Begin a transaction on a pooled connection held for the session helpers below.
    ///
    /// Phase 4 slice: commit/rollback use explicit SQL on a checked-out connection
    /// via [`with_connection`]. Prefer that for multi-statement transactions.
    pub fn with_connection<R>(
        &self,
        f: impl FnOnce(&mut postgres::Client) -> Result<R>,
    ) -> Result<R> {
        let mut client = self.pool.get().map_err(|e| Error::Pool(e.to_string()))?;
        f(&mut client)
    }

    /// Run `f` inside `BEGIN` / `COMMIT`, rolling back on error.
    pub fn transaction<R>(&self, f: impl FnOnce(&mut postgres::Client) -> Result<R>) -> Result<R> {
        self.with_connection(|client| {
            client.batch_execute("BEGIN")?;
            match f(client) {
                Ok(v) => {
                    client.batch_execute("COMMIT")?;
                    Ok(v)
                }
                Err(e) => {
                    let _ = client.batch_execute("ROLLBACK");
                    Err(e)
                }
            }
        })
    }

    /// Install authority commit counter + outbox tables (idempotent).
    pub fn ensure_authority_outbox(&self) -> Result<()> {
        self.with_connection(outbox::ensure_schema)
    }

    /// Current authority [`CommitToken`].
    pub fn current_commit_token(&self) -> Result<CommitToken> {
        self.with_connection(outbox::current_token)
    }

    /// Authority transaction with durable outbox append (atomic with mutations).
    pub fn authority_commit<R>(
        &self,
        f: impl FnOnce(&mut AuthorityTxn<'_>) -> Result<R>,
    ) -> Result<(R, CommitToken)> {
        self.with_connection(|client| outbox::authority_commit(client, f))
    }

    /// Poll durable outbox events after a sequence.
    pub fn outbox_after(&self, after_seq: u64, limit: usize) -> Result<Vec<OutboxRecord>> {
        self.with_connection(|client| outbox::outbox_after(client, after_seq, limit))
    }

    /// Outbox backlog size.
    pub fn outbox_backlog(&self) -> Result<u64> {
        self.with_connection(outbox::outbox_backlog)
    }
}

fn parse_vos(vos_schema: &str) -> Result<vos::ast::Document> {
    vos::parser::parse_document(vos_schema).map_err(|d| {
        Error::Vos(format!(
            "parse schema: {}",
            d.errors
                .first()
                .map(|e| e.message.as_str())
                .unwrap_or("unknown")
        ))
    })
}

/// Adapter errors.
#[derive(Debug)]
pub enum Error {
    /// Connection / config parse failure.
    Config(String),
    /// Pool checkout failure.
    Pool(String),
    /// PostgreSQL driver failure.
    Postgres(postgres::Error),
    /// VOS schema parse / semantic issues.
    Vos(String),
    /// Policy / adopt blocker / destructive without review.
    Policy(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(s) | Self::Pool(s) | Self::Vos(s) | Self::Policy(s) => write!(f, "{s}"),
            Self::Postgres(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Postgres(e) => Some(e),
            _ => None,
        }
    }
}

impl From<postgres::Error> for Error {
    fn from(value: postgres::Error) -> Self {
        Self::Postgres(value)
    }
}

/// Result alias.
pub type Result<T> = std::result::Result<T, Error>;
