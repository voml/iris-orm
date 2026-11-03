//! Stateful Iris sessions for Node hosts.
//!
//! The session state machine, profile dispatch and diagnostic classification
//! live in `iris-host` so the Node and Python bindings cannot drift apart.
//! This module only translates that surface into napi-rs values.

use iris_host::{ErrorEnvelope, ExecuteOutcome, HostSession};
use napi::bindgen_prelude::*;
use napi_derive::napi;

fn envelope_error(err: ErrorEnvelope) -> Error {
    Error::from_reason(err.message)
}

/// Execute VOS against the bound adapter.
#[napi(object)]
pub struct ExecuteResult {
    pub ok: bool,
    /// JSON array of row objects.
    pub rows_json: String,
    pub error: Option<String>,
    /// Stable diagnostic code when `ok` is false.
    pub error_code: Option<String>,
}

fn to_result(outcome: ExecuteOutcome) -> ExecuteResult {
    ExecuteResult { ok: outcome.ok, rows_json: outcome.rows_json(), error: outcome.error_message(), error_code: outcome.error_code() }
}

/// Iris session bound to a reference or foreign adapter.
#[napi]
pub struct MemorySession {
    inner: Option<HostSession>,
}

impl MemorySession {
    fn session(&self) -> Result<&HostSession> {
        match &self.inner {
            Some(inner) if !inner.is_closed() => Ok(inner),
            _ => Err(Error::from_reason("session closed")),
        }
    }

    pub(crate) fn open_memory() -> Self {
        Self { inner: Some(HostSession::open_memory()) }
    }

    pub(crate) fn open_sqlite(path: String) -> Result<Self> {
        HostSession::open_sqlite(&path).map(|inner| Self { inner: Some(inner) }).map_err(envelope_error)
    }

    pub(crate) fn open_postgres(url: String) -> Result<Self> {
        HostSession::open_postgres(&url).map(|inner| Self { inner: Some(inner) }).map_err(envelope_error)
    }

    pub(crate) fn open_yydb(endpoint: String) -> Result<Self> {
        HostSession::open_yydb(&endpoint).map(|inner| Self { inner: Some(inner) }).map_err(envelope_error)
    }

    pub(crate) fn open_mysql(url: String) -> Result<Self> {
        HostSession::open_mysql(&url).map(|inner| Self { inner: Some(inner) }).map_err(envelope_error)
    }

    pub(crate) fn open_project(config_path: String, source: String) -> Result<Self> {
        HostSession::open_project(&config_path, &source).map(|inner| Self { inner: Some(inner) }).map_err(envelope_error)
    }
}

#[napi]
impl MemorySession {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self::open_memory()
    }

    /// Plan + execute VOS DML (returns rows). Aligns with generated `db.$query`.
    #[napi]
    pub fn query(&self, source: String, parameters_json: Option<String>) -> Result<ExecuteResult> {
        let session = self.session()?;
        Ok(to_result(session.query(&source, parameters_json.as_deref())))
    }

    /// Execute unit-valued / DDL-shaped VOS. Aligns with generated `db.$execute`.
    #[napi]
    pub fn execute(&self, source: String, parameters_json: Option<String>) -> Result<ExecuteResult> {
        let session = self.session()?;
        Ok(to_result(session.execute(&source, parameters_json.as_deref())))
    }

    /// Plan + execute VOS DML (legacy name).
    #[napi]
    pub fn execute_vos(&self, source: String, parameters_json: Option<String>) -> Result<ExecuteResult> {
        self.query(source, parameters_json)
    }

    /// Execute a structured Iris operation JSON payload (generated client ABI).
    #[napi(js_name = executeOperation)]
    pub fn execute_operation(&self, request_json: String) -> Result<ExecuteResult> {
        let session = self.session()?;
        Ok(to_result(session.execute_operation(&request_json)))
    }

    /// Apply managed-push schema to a YYDB or SQLite session.
    #[napi]
    pub fn managed_push(&self, schema: String) -> Result<()> {
        self.session()?.managed_push(&schema).map_err(envelope_error)
    }

    /// Open a transaction on YYDB or SQLite sessions.
    #[napi]
    pub fn begin(&self) -> Result<ExecuteResult> {
        let session = self.session()?;
        Ok(to_result(session.begin()))
    }

    /// Commit the active transaction.
    #[napi]
    pub fn commit(&self) -> Result<ExecuteResult> {
        let session = self.session()?;
        Ok(to_result(session.commit()))
    }

    /// Roll back the active transaction.
    #[napi]
    pub fn rollback(&self) -> Result<ExecuteResult> {
        let session = self.session()?;
        Ok(to_result(session.rollback()))
    }

    /// SQLite engine version for the active sqlite session.
    #[napi(js_name = sqliteEngineVersion)]
    pub fn sqlite_engine_version(&self) -> Result<String> {
        self.session()?.sqlite_engine_version().map_err(envelope_error)
    }

    /// SQLite engine source id for the active sqlite session.
    #[napi(js_name = sqliteSourceId)]
    pub fn sqlite_source_id(&self) -> Result<String> {
        self.session()?.sqlite_source_id().map_err(envelope_error)
    }

    /// Close the session, releasing the bound datasource.
    ///
    /// Dropping the store is what frees a file-backed datasource's writer
    /// lock, so `close` must do more than flip a flag: after it returns the
    /// same database file can be opened again from this process.
    #[napi]
    pub fn close(&mut self) {
        if let Some(inner) = self.inner.as_mut() {
            inner.close();
        }
        self.inner = None;
    }
}
