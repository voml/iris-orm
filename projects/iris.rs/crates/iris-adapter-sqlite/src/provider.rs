//! Native SQLite execution surface consumed by this adapter.
//!
//! Upstream `sqlite-provider` / `sqlite-provider-rusqlite` are no longer
//! published, so the adapter owns the provider it consumes: a bundled SQLite
//! engine reached through `rusqlite`, typed storage classes, statement results,
//! and catalog snapshots. SQL text stays private to this crate.

use std::path::Path;

use rusqlite::{Connection, params_from_iter, types::ValueRef};

/// One SQLite storage-class value with lossless integer and binary payloads.
#[derive(Debug, Clone, PartialEq)]
pub enum SqliteValue {
    /// SQL NULL.
    Null,
    /// Signed SQLite INTEGER.
    Integer(i64),
    /// SQLite REAL.
    Real(f64),
    /// SQLite TEXT bytes.
    Text(Vec<u8>),
    /// SQLite BLOB bytes.
    Blob(Vec<u8>),
}

impl SqliteValue {
    /// Returns the storage-class label.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Integer(_) => "integer",
            Self::Real(_) => "real",
            Self::Text(_) => "text",
            Self::Blob(_) => "blob",
        }
    }
}

impl std::fmt::Display for SqliteValue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Null => Ok(()),
            Self::Integer(value) => write!(formatter, "{value}"),
            Self::Real(value) => write!(formatter, "{value}"),
            Self::Text(value) => formatter.write_str(&String::from_utf8_lossy(value)),
            Self::Blob(value) => {
                formatter.write_str("x'")?;
                for byte in value {
                    write!(formatter, "{byte:02x}")?;
                }
                formatter.write_str("'")
            }
        }
    }
}

/// Result of executing exactly one SQL statement.
#[derive(Debug, Clone, PartialEq)]
pub struct StatementResult {
    /// Result column labels in order.
    pub columns: Vec<String>,
    /// Materialized rows preserving SQLite storage classes.
    pub rows: Vec<Vec<SqliteValue>>,
    /// Rows changed by the statement.
    pub changes: u64,
    /// Last insert rowid for the connection after execution.
    pub last_insert_rowid: i64,
}

/// One observed column in the physical catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogColumn {
    /// Physical column name.
    pub name: String,
    /// Declared SQLite type text.
    pub type_name: String,
    /// Whether the column accepts NULL.
    pub nullable: bool,
    /// Whether the column participates in the primary key.
    pub primary_key: bool,
}

/// One observed physical table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogTable {
    /// Physical table name.
    pub name: String,
    /// Observed columns in physical order.
    pub columns: Vec<CatalogColumn>,
}

/// Observed catalog snapshot returned by [`RusqliteProvider::inspect_catalog`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CatalogSnapshot {
    /// Observed user tables (SQLite internal tables excluded).
    pub tables: Vec<CatalogTable>,
}

impl CatalogSnapshot {
    /// Find an observed table by physical name.
    pub fn table(&self, name: &str) -> Option<&CatalogTable> {
        self.tables.iter().find(|table| table.name == name)
    }
}

/// Structured provider failure.
#[derive(Debug)]
pub struct SqliteError {
    message: String,
}

impl SqliteError {
    /// Build a policy or contract failure without a native result code.
    pub fn policy(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }
}

impl std::fmt::Display for SqliteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for SqliteError {}

impl From<rusqlite::Error> for SqliteError {
    fn from(value: rusqlite::Error) -> Self {
        Self { message: value.to_string() }
    }
}

impl From<SqliteError> for rusqlite::Error {
    fn from(value: SqliteError) -> Self {
        rusqlite::Error::ToSqlConversionFailure(Box::new(value))
    }
}

/// A bundled SQLite engine connection.
pub struct RusqliteProvider {
    connection: Connection,
}

impl std::fmt::Debug for RusqliteProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("RusqliteProvider").finish_non_exhaustive()
    }
}

impl RusqliteProvider {
    /// Open an in-memory SQLite database.
    pub fn open_in_memory() -> Result<Self, SqliteError> {
        Self::configure(Connection::open_in_memory()?)
    }

    /// Open or create a file-backed SQLite database.
    pub fn open(path: &Path) -> Result<Self, SqliteError> {
        Self::configure(Connection::open(path)?)
    }

    fn configure(connection: Connection) -> Result<Self, SqliteError> {
        connection.execute_batch("PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;")?;
        Ok(Self { connection })
    }

    /// Returns `SELECT sqlite_version()` from the active engine.
    pub fn sqlite_version(&self) -> Result<String, SqliteError> {
        Ok(self.connection.query_row("SELECT sqlite_version()", [], |row| row.get(0))?)
    }

    /// Returns `SELECT sqlite_source_id()` from the active engine.
    pub fn source_id(&self) -> Result<String, SqliteError> {
        Ok(self.connection.query_row("SELECT sqlite_source_id()", [], |row| row.get(0))?)
    }

    /// Executes exactly one statement with positional bind parameters.
    pub fn execute_one(&self, sql: &str, params: &[SqliteValue]) -> Result<StatementResult, SqliteError> {
        let mut statement = self.connection.prepare(sql)?;
        let columns: Vec<String> = statement.column_names().into_iter().map(str::to_owned).collect();
        let bound: Vec<rusqlite::types::Value> = params.iter().map(bound_value).collect();
        let mut rows = Vec::new();
        {
            let mut cursor = statement.query(params_from_iter(bound))?;
            while let Some(row) = cursor.next()? {
                let mut values = Vec::with_capacity(columns.len());
                for index in 0..columns.len() {
                    values.push(observed_value(row.get_ref(index)?));
                }
                rows.push(values);
            }
        }
        drop(statement);
        Ok(StatementResult { columns, rows, changes: self.connection.changes(), last_insert_rowid: self.connection.last_insert_rowid() })
    }

    /// Executes a multi-statement batch and discards result rows.
    pub fn execute_batch(&self, sql: &str) -> Result<(), SqliteError> {
        self.connection.execute_batch(sql)?;
        Ok(())
    }

    /// Starts an immediate write transaction.
    pub fn begin_immediate(&self) -> Result<(), SqliteError> {
        self.execute_batch("BEGIN IMMEDIATE")
    }

    /// Commits the active transaction.
    pub fn commit(&self) -> Result<(), SqliteError> {
        self.execute_batch("COMMIT")
    }

    /// Rolls back the active transaction.
    pub fn rollback(&self) -> Result<(), SqliteError> {
        self.execute_batch("ROLLBACK")
    }

    /// Inspects live catalog metadata through SQLite metadata queries.
    pub fn inspect_catalog(&self) -> Result<CatalogSnapshot, SqliteError> {
        let mut tables = Vec::new();
        for name in self.table_names()? {
            let quoted = name.replace('"', "\"\"");
            let mut statement = self.connection.prepare(&format!("PRAGMA table_info(\"{quoted}\")"))?;
            let columns = statement.query_map([], |row| {
                Ok(CatalogColumn {
                    name: row.get(1)?,
                    type_name: row.get(2)?,
                    nullable: row.get::<_, i64>(3)? == 0,
                    primary_key: row.get::<_, i64>(5)? > 0,
                })
            })?;
            tables.push(CatalogTable { name, columns: columns.collect::<Result<Vec<_>, _>>()? });
        }
        Ok(CatalogSnapshot { tables })
    }

    fn table_names(&self) -> Result<Vec<String>, SqliteError> {
        let mut statement =
            self.connection.prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

fn bound_value(value: &SqliteValue) -> rusqlite::types::Value {
    match value {
        SqliteValue::Null => rusqlite::types::Value::Null,
        SqliteValue::Integer(integer) => rusqlite::types::Value::Integer(*integer),
        SqliteValue::Real(real) => rusqlite::types::Value::Real(*real),
        SqliteValue::Text(bytes) => rusqlite::types::Value::Text(String::from_utf8_lossy(bytes).into_owned()),
        SqliteValue::Blob(bytes) => rusqlite::types::Value::Blob(bytes.clone()),
    }
}

fn observed_value(value: ValueRef<'_>) -> SqliteValue {
    match value {
        ValueRef::Null => SqliteValue::Null,
        ValueRef::Integer(integer) => SqliteValue::Integer(integer),
        ValueRef::Real(real) => SqliteValue::Real(real),
        ValueRef::Text(bytes) => SqliteValue::Text(bytes.to_vec()),
        ValueRef::Blob(bytes) => SqliteValue::Blob(bytes.to_vec()),
    }
}
