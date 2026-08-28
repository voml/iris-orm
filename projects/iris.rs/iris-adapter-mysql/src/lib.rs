//! Isolated MySQL foreign-store adapter.
//!
//! Backend commands stay **private**. Catalog inspect, type mapping, and DDL
//! emission are MySQL-specific — not shared with PostgreSQL/SQLite adapters.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod catalog;
mod execute;
mod migrate;
mod schema_map;
mod uuid_util;

use std::collections::HashSet;

use iris_ir::{IrVersion, PhysicalPlan};
use iris_types::{
    CapabilitySet, DriftReport, LogicalMigrationPlan, MappingManifest, ObservedCatalog, QueryCaps,
    Row, RowWrite, WriteCaps,
};
use mysql::Pool;
use mysql::prelude::*;
use vos::ast::Document;

pub use catalog::{adopt_plan, classify_type};
pub use migrate::{PushReport, apply_push, plan_push};
pub use schema_map::collect_uuid_fields;
/// Connection handle for generated `Txn` (adapter-internal checkout; not an app pool).
pub use mysql::PooledConn;

/// Adapter identifier.
pub const BACKEND_ID: &str = "mysql";

/// Adapter crate version label.
pub const ADAPTER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// MySQL foreign datasource backed by the mysql crate pool.
pub struct MysqlSource {
    pool: Pool,
    uuid_fields: HashSet<(String, String)>,
}

impl std::fmt::Debug for MysqlSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MysqlSource")
            .field("backend", &BACKEND_ID)
            .finish_non_exhaustive()
    }
}

impl MysqlSource {
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

    /// Connect using a mysql URL (`mysql://user:pass@host:3306/db`).
    pub fn connect(url: &str) -> Result<Self> {
        let opts = mysql::Opts::from_url(url).map_err(|e| Error::Config(e.to_string()))?;
        let pool = Pool::new(opts).map_err(Error::Mysql)?;
        Ok(Self {
            pool,
            uuid_fields: HashSet::new(),
        })
    }

    /// Register VOS schema so `uuid` columns encode/decode as MySQL `BINARY(16)`.
    pub fn with_vos_schema(mut self, vos_schema: &str) -> Result<Self> {
        let document = parse_vos(vos_schema)?;
        self.uuid_fields = collect_uuid_fields(&document);
        Ok(self)
    }

    /// Register uuid columns from generated bindings (`UUID_FIELDS`) without re-parsing `.iris`.
    pub fn with_uuid_fields<I, T, F>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = (T, F)>,
        T: Into<String>,
        F: Into<String>,
    {
        self.uuid_fields = fields
            .into_iter()
            .map(|(table, field)| (table.into(), field.into()))
            .collect();
        self
    }

    /// Register uuid columns from a parsed VOS document.
    pub fn with_schema_document(mut self, document: &Document) -> Self {
        self.uuid_fields = collect_uuid_fields(document);
        self
    }

    /// Pool checkout smoke (connection-pool conformance).
    pub fn ping(&self) -> Result<()> {
        let mut conn = self.pool.get_conn()?;
        conn.query_drop("SELECT 1")?;
        Ok(())
    }

    /// Inspect the foreign catalog (Adopt Existing input).
    pub fn inspect(&self) -> Result<ObservedCatalog> {
        let mut conn = self.pool.get_conn()?;
        catalog::inspect_catalog(&mut conn)
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
        let mut conn = self.pool.get_conn()?;
        migrate::apply_push(&mut conn, plan, &document)
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
        let mut conn = self.pool.get_conn()?;
        self.execute_plan_on(&mut conn, plan)
    }

    /// Execute a plan on an existing connection (same session / transaction).
    pub fn execute_plan_on(
        &self,
        conn: &mut mysql::PooledConn,
        plan: &PhysicalPlan,
    ) -> Result<Vec<Row>> {
        if plan.is_rejected() {
            return Err(Error::Policy(
                plan.rejection_note().unwrap_or("plan rejected").to_string(),
            ));
        }
        execute::execute_plan(conn, plan, &self.uuid_fields)
    }

    /// Insert a row.
    pub fn insert(&self, write: &RowWrite) -> Result<()> {
        let mut conn = self.pool.get_conn()?;
        self.insert_on(&mut conn, write)
    }

    /// Insert on an existing connection (same session / transaction).
    pub fn insert_on(&self, conn: &mut mysql::PooledConn, write: &RowWrite) -> Result<()> {
        execute::insert_row(conn, write, &self.uuid_fields)
    }

    /// Update by primary key.
    pub fn update(&self, write: &RowWrite) -> Result<u64> {
        let mut conn = self.pool.get_conn()?;
        self.update_on(&mut conn, write)
    }

    /// Update on an existing connection (same session / transaction).
    pub fn update_on(&self, conn: &mut mysql::PooledConn, write: &RowWrite) -> Result<u64> {
        execute::update_row(conn, write, &self.uuid_fields)
    }

    /// Delete by primary key.
    pub fn delete(&self, table: &str, primary_key: &str, key: &iris_types::Value) -> Result<u64> {
        let mut conn = self.pool.get_conn()?;
        self.delete_on(&mut conn, table, primary_key, key)
    }

    /// Delete on an existing connection (same session / transaction).
    pub fn delete_on(
        &self,
        conn: &mut mysql::PooledConn,
        table: &str,
        primary_key: &str,
        key: &iris_types::Value,
    ) -> Result<u64> {
        execute::delete_row(conn, table, primary_key, key, &self.uuid_fields)
    }

    /// Run `f` inside a transaction, rolling back on error.
    ///
    /// Prefer generated `Txn` (or these `*_on` helpers) inside `f`. Calling
    /// [`Self::execute_plan`] / [`Self::insert`] from `f` checks out another
    /// connection from the adapter pool and will not participate in this transaction.
    /// The pool itself stays inside this adapter — apps should not build a second pool.
    pub fn transaction<R>(&self, f: impl FnOnce(&mut mysql::PooledConn) -> Result<R>) -> Result<R> {
        let mut conn = self.pool.get_conn()?;
        conn.query_drop("START TRANSACTION")?;
        match f(&mut conn) {
            Ok(v) => {
                conn.query_drop("COMMIT")?;
                Ok(v)
            }
            Err(e) => {
                let _ = conn.query_drop("ROLLBACK");
                Err(e)
            }
        }
    }

    /// Like [`Self::transaction`], but **always** `ROLLBACK` — for integration
    /// tests against a shared database (insert fixtures, assert, leave no residue).
    pub fn with_rollback<R>(
        &self,
        f: impl FnOnce(&mut mysql::PooledConn) -> Result<R>,
    ) -> Result<R> {
        let mut conn = self.pool.get_conn()?;
        conn.query_drop("START TRANSACTION")?;
        let result = f(&mut conn);
        let _ = conn.query_drop("ROLLBACK");
        result
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
    /// Connection / URL parse failure.
    Config(String),
    /// MySQL driver failure.
    Mysql(mysql::Error),
    /// VOS schema parse / semantic issues.
    Vos(String),
    /// Policy / adopt blocker / destructive without review.
    Policy(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(s) | Self::Vos(s) | Self::Policy(s) => write!(f, "{s}"),
            Self::Mysql(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Mysql(e) => Some(e),
            _ => None,
        }
    }
}

impl From<mysql::Error> for Error {
    fn from(value: mysql::Error) -> Self {
        Self::Mysql(value)
    }
}

/// Result alias.
pub type Result<T> = std::result::Result<T, Error>;
