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

use std::{path::Path, sync::Mutex};

use iris::{
    CapabilitySet, DatasourceKind, Iris, Planner, ReferenceStore, Row,
    project::{expand_endpoint, load_project, read_schema},
    resolve_path,
};
use iris_adapter_mysql::{MysqlSource, MysqlTransaction};
use iris_adapter_postgres::{PostgresSource, PostgresTransaction};
use iris_adapter_sqlite::SqliteSource;
use iris_connector_yydb::YydbSource;
use iris_wasm::{bind, wire::rows_to_json};
use serde_json::{Value as JsonValue, json};

pub mod operation;

/// A unique key or primary key already exists.
pub const CODE_UNIQUE_CONFLICT: &str = "IRIS-UNIQUE-CONFLICT";
/// A unique `where` matched no row.
pub const CODE_NOT_FOUND: &str = "IRIS-NOT-FOUND";
/// A vector value does not match the schema dimension.
pub const CODE_VECTOR_DIMENSION: &str = "IRIS-VECTOR-DIMENSION";
/// Generated artifacts and the bound catalog disagree.
pub const CODE_SCHEMA_MISMATCH: &str = "IRIS-SCHEMA-MISMATCH";
/// The DSN could not be parsed, authenticated or connected.
pub const CODE_CONNECTION_FAILED: &str = "IRIS-CONNECTION-FAILED";
/// An argument, condition object or DSN shape is wrong.
pub const CODE_INVALID_ARGUMENT: &str = "IRIS-INVALID-ARGUMENT";
/// Begin, commit or rollback failed, including unsupported profiles.
pub const CODE_TRANSACTION_FAILED: &str = "IRIS-TRANSACTION-FAILED";
/// An unclassified internal failure.
pub const CODE_INTERNAL: &str = "IRIS-INTERNAL";

/// The complete author-facing code table. Nothing outside this list may
/// appear as `ErrorEnvelope::code`.
pub const AUTHOR_CODES: &[&str] = &[
    CODE_UNIQUE_CONFLICT,
    CODE_NOT_FOUND,
    CODE_VECTOR_DIMENSION,
    CODE_SCHEMA_MISMATCH,
    CODE_CONNECTION_FAILED,
    CODE_INVALID_ARGUMENT,
    CODE_TRANSACTION_FAILED,
    CODE_INTERNAL,
];

/// Diagnostic only: the host session has been closed.
pub const DIAG_SESSION_CLOSED: &str = "IRIS-SESSION-CLOSED";
/// Diagnostic only: a parameter payload could not be bound.
pub const DIAG_PARAM_ENCODING: &str = "IRIS-PARAM-ENCODING";
/// Diagnostic only: the structured operation payload is invalid.
pub const DIAG_OPERATION_INVALID: &str = "IRIS-OPERATION-INVALID";
/// Diagnostic only: the bound backend rejected the execution.
pub const DIAG_EXEC_FAILED: &str = "IRIS-EXEC-FAILED";
/// Diagnostic only: the active profile has no transaction control.
pub const DIAG_TRANSACTION_UNSUPPORTED: &str = "IRIS-TRANSACTION-UNSUPPORTED";
/// Diagnostic only: a transaction is already open on this session.
pub const DIAG_TRANSACTION_ALREADY_OPEN: &str = "IRIS-TRANSACTION-ALREADY-OPEN";
/// Diagnostic only: no transaction is open on this session.
pub const DIAG_TRANSACTION_NOT_OPEN: &str = "IRIS-TRANSACTION-NOT-OPEN";
/// Diagnostic only: managed push is not available on this profile.
pub const DIAG_MANAGED_PUSH_UNSUPPORTED: &str = "IRIS-MANAGED-PUSH-UNSUPPORTED";
/// Diagnostic only: the selected backend cannot represent a schema capability.
pub const DIAG_SCHEMA_VECTOR_UNSUPPORTED: &str = "IRIS-SCHEMA-VECTOR-UNSUPPORTED";

/// Structured failure shared by every host binding.
///
/// `kind` and `code` are the author-facing contract: both language bindings
/// pass them through unchanged and authors branch on them. `diagnostic_code`
/// carries the internal code (`IRIS-PLAN-REJECTED`, `23505`, `yydb.*`) for
/// logs and tests and is not part of that contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorEnvelope {
    /// Semantic class (`unique_conflict`, `not_found`, `invalid_argument`, …).
    pub kind: String,
    /// Stable author-facing code.
    pub code: String,
    /// Human-readable explanation.
    pub message: String,
    /// Internal diagnostic code, if any.
    pub diagnostic_code: Option<String>,
}

fn kind_for(code: &str) -> &'static str {
    match code {
        CODE_UNIQUE_CONFLICT => "unique_conflict",
        CODE_NOT_FOUND => "not_found",
        CODE_VECTOR_DIMENSION => "vector_dimension_mismatch",
        CODE_SCHEMA_MISMATCH => "schema_mismatch",
        CODE_CONNECTION_FAILED => "connection_failed",
        CODE_INVALID_ARGUMENT => "invalid_argument",
        CODE_TRANSACTION_FAILED => "transaction_failed",
        _ => "internal",
    }
}

impl ErrorEnvelope {
    /// Build an envelope from an author-facing code and a display message.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        let code = code.into();
        let kind = kind_for(&code).to_string();
        Self { kind, code, message: message.into(), diagnostic_code: None }
    }

    /// Attach the internal diagnostic code that produced this failure.
    pub fn with_diagnostic(mut self, diagnostic: impl Into<String>) -> Self {
        self.diagnostic_code = Some(diagnostic.into());
        self
    }

    /// Unclassified failure with the original internal code as diagnostic.
    pub fn internal(diagnostic: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(CODE_INTERNAL, message).with_diagnostic(diagnostic)
    }

    /// Author-argument failure with the original internal code as diagnostic.
    pub fn invalid_argument(diagnostic: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(CODE_INVALID_ARGUMENT, message).with_diagnostic(diagnostic)
    }

    /// JSON form used on the wire.
    pub fn to_json(&self) -> JsonValue {
        json!({
            "kind": self.kind,
            "code": self.code,
            "message": self.message,
            "diagnosticCode": self.diagnostic_code,
        })
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

    /// Semantic class of the failure, if any.
    pub fn error_kind(&self) -> Option<String> {
        self.error.as_ref().map(|err| err.kind.clone())
    }

    /// Internal diagnostic code of the failure, if any.
    pub fn error_diagnostic_code(&self) -> Option<String> {
        self.error.as_ref().and_then(|err| err.diagnostic_code.clone())
    }
}

fn from_iris_error(err: iris_types::Error) -> ErrorEnvelope {
    match err {
        // A rejected plan means the requested shape does not fit the schema or
        // the bound catalog; the original code stays as a diagnostic.
        iris_types::Error::Diagnostic(diag) => ErrorEnvelope::invalid_argument(diag.code, diag.message),
        iris_types::Error::Ir(inner) => ErrorEnvelope::internal(DIAG_EXEC_FAILED, inner.to_string()),
        iris_types::Error::Runtime(message) => ErrorEnvelope::internal(DIAG_EXEC_FAILED, message),
    }
}

fn classify_yydb_code(code: &str) -> &'static str {
    match code {
        "yydb.unique.violation" => CODE_UNIQUE_CONFLICT,
        "yydb.transaction.conflict" => CODE_TRANSACTION_FAILED,
        "yydb.schema.vector_dimension_exceeded" => CODE_VECTOR_DIMENSION,
        "yydb.schema.conflict" | "yydb.schema.invalid" | "yydb.schema.invalidated" => CODE_SCHEMA_MISMATCH,
        "yydb.session.closed" => CODE_INVALID_ARGUMENT,
        "yydb.io" => CODE_CONNECTION_FAILED,
        _ => CODE_INTERNAL,
    }
}

fn from_yydb_error(err: iris_connector_yydb::Error) -> ErrorEnvelope {
    match err {
        iris_connector_yydb::Error::Yydb(inner) => {
            ErrorEnvelope::new(classify_yydb_code(inner.code()), inner.to_string()).with_diagnostic(inner.code())
        }
        iris_connector_yydb::Error::NotReady(report) => ErrorEnvelope::new(CODE_SCHEMA_MISMATCH, report.message).with_diagnostic(report.code),
        iris_connector_yydb::Error::Runtime(message) => ErrorEnvelope::internal(DIAG_EXEC_FAILED, message),
        iris_connector_yydb::Error::Policy(message) => ErrorEnvelope::new(CODE_SCHEMA_MISMATCH, message).with_diagnostic("IRIS-SCHEMA-POLICY"),
    }
}

fn classify_sqlite_message(message: &str) -> &'static str {
    let lower = message.to_ascii_lowercase();
    if lower.contains("unique constraint failed") || lower.contains("primary key") {
        CODE_UNIQUE_CONFLICT
    }
    else if lower.contains("no such table") || lower.contains("no such column") {
        CODE_SCHEMA_MISMATCH
    }
    else if lower.contains("unable to open database file") || lower.contains("database is locked") {
        CODE_CONNECTION_FAILED
    }
    else {
        CODE_INTERNAL
    }
}

fn from_sqlite_error(err: iris_adapter_sqlite::Error) -> ErrorEnvelope {
    let message = err.to_string();
    let code = classify_sqlite_message(&message);
    let diagnostic = match code {
        CODE_UNIQUE_CONFLICT => "sqlite.unique",
        CODE_SCHEMA_MISMATCH => "sqlite.schema",
        CODE_CONNECTION_FAILED => "sqlite.io",
        _ => "sqlite.error",
    };
    ErrorEnvelope::new(code, message).with_diagnostic(diagnostic)
}

fn postal_state_of(err: &postgres::Error) -> Option<String> {
    err.code().map(|state| state.code().to_string())
}

fn classify_sql_state(state: Option<&str>) -> &'static str {
    match state {
        Some("23505") => CODE_UNIQUE_CONFLICT,
        Some("22P02") | Some("22001") | Some("22003") => CODE_INVALID_ARGUMENT,
        Some("42P01") | Some("42703") | Some("42P07") | Some("42710") => CODE_SCHEMA_MISMATCH,
        Some(code) if code.starts_with("08") => CODE_CONNECTION_FAILED,
        _ => CODE_INTERNAL,
    }
}

fn from_postgres_error(err: iris_adapter_postgres::Error) -> ErrorEnvelope {
    match &err {
        iris_adapter_postgres::Error::Config(message) | iris_adapter_postgres::Error::Pool(message) => {
            ErrorEnvelope::new(CODE_CONNECTION_FAILED, message).with_diagnostic("postgres.connection")
        }
        iris_adapter_postgres::Error::Postgres(inner) => {
            let state = postal_state_of(inner);
            let code = classify_sql_state(state.as_deref());
            let diagnostic = state.clone().unwrap_or_else(|| "postgres.error".to_string());
            ErrorEnvelope::new(code, inner.to_string()).with_diagnostic(diagnostic)
        }
        iris_adapter_postgres::Error::Vos(message) => ErrorEnvelope::new(CODE_SCHEMA_MISMATCH, message).with_diagnostic("postgres.vos"),
        iris_adapter_postgres::Error::Policy(message) => {
            ErrorEnvelope::new(CODE_SCHEMA_MISMATCH, message).with_diagnostic("IRIS-SCHEMA-POLICY")
        }
    }
}

fn classify_mysql_code(code: u16) -> &'static str {
    match code {
        1062 | 1586 => CODE_UNIQUE_CONFLICT,
        1048 | 1364 | 1136 => CODE_INVALID_ARGUMENT,
        1146 | 1054 | 1050 | 1060 => CODE_SCHEMA_MISMATCH,
        1045 | 1049 | 2003 | 2005 | 2013 => CODE_CONNECTION_FAILED,
        _ => CODE_INTERNAL,
    }
}

fn from_mysql_error(err: iris_adapter_mysql::Error) -> ErrorEnvelope {
    match &err {
        iris_adapter_mysql::Error::Config(message) => ErrorEnvelope::new(CODE_CONNECTION_FAILED, message).with_diagnostic("mysql.connection"),
        iris_adapter_mysql::Error::Mysql(inner) => {
            let server_code = match inner {
                mysql::Error::MySqlError(server) => Some(server.code),
                _ => None,
            };
            let code = server_code.map(classify_mysql_code).unwrap_or(CODE_INTERNAL);
            let diagnostic = server_code.map(|code| code.to_string()).unwrap_or_else(|| "mysql.error".to_string());
            ErrorEnvelope::new(code, inner.to_string()).with_diagnostic(diagnostic)
        }
        iris_adapter_mysql::Error::Vos(message) => ErrorEnvelope::new(CODE_SCHEMA_MISMATCH, message).with_diagnostic("mysql.vos"),
        iris_adapter_mysql::Error::Policy(message) => ErrorEnvelope::new(CODE_SCHEMA_MISMATCH, message).with_diagnostic("IRIS-SCHEMA-POLICY"),
    }
}

fn closed() -> ErrorEnvelope {
    ErrorEnvelope::invalid_argument(DIAG_SESSION_CLOSED, "session closed")
}

fn transaction_unsupported(profile: &str) -> ErrorEnvelope {
    ErrorEnvelope::new(CODE_TRANSACTION_FAILED, format!("{profile} sessions have no transaction control"))
        .with_diagnostic(DIAG_TRANSACTION_UNSUPPORTED)
}

fn transaction_already_open() -> ErrorEnvelope {
    ErrorEnvelope::invalid_argument(DIAG_TRANSACTION_ALREADY_OPEN, "a transaction is already open on this session")
}

fn transaction_not_open() -> ErrorEnvelope {
    ErrorEnvelope::invalid_argument(DIAG_TRANSACTION_NOT_OPEN, "no transaction is open on this session")
}

fn lock_failed() -> ErrorEnvelope {
    ErrorEnvelope::internal(DIAG_EXEC_FAILED, "session transaction lock is poisoned")
}

/// Reject a schema the selected SQL backend cannot represent.
///
/// Vector search is a yydb capability in `/v0`, so building a MySQL or
/// PostgreSQL client from a schema that declares `vector` fields fails here,
/// before any call, instead of surfacing later as an argument error.
fn reject_unsupported_schema(profile: &str, schema: Option<&str>) -> Result<(), ErrorEnvelope> {
    let Some(schema) = schema
    else {
        return Ok(());
    };
    if schema.trim().is_empty() || !schema_declares_vector(schema)? {
        return Ok(());
    }
    Err(ErrorEnvelope::new(
        CODE_SCHEMA_MISMATCH,
        format!("{profile} cannot represent the `vector` fields of this schema: vector search is only available on yydb profiles"),
    )
    .with_diagnostic(DIAG_SCHEMA_VECTOR_UNSUPPORTED))
}

fn schema_declares_vector(schema: &str) -> Result<bool, ErrorEnvelope> {
    use vos::syntax::VosDeclarationKind;
    let input = vos::parse_oak(schema)
        .map_err(|message| ErrorEnvelope::new(CODE_SCHEMA_MISMATCH, message).with_diagnostic(DIAG_SCHEMA_VECTOR_UNSUPPORTED))?;
    let root = input.into_oak();
    let declarations = root
        .declarations
        .iter()
        .filter(|declaration| matches!(declaration.kind, VosDeclarationKind::Namespace | VosDeclarationKind::Table | VosDeclarationKind::Class))
        .cloned()
        .collect();
    let projection = vos::ContractInput::from_oak(vos::syntax::VosRoot { source: root.source, syntax: root.syntax, declarations })
        .project_schema()
        .map_err(|diagnostics| {
            let reason =
                diagnostics.iter().map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message)).collect::<Vec<_>>().join("\n");
            ErrorEnvelope::new(CODE_SCHEMA_MISMATCH, reason).with_diagnostic(DIAG_SCHEMA_VECTOR_UNSUPPORTED)
        })?;
    Ok(projection.types.iter().flat_map(|item| item.fields.iter()).any(|field| type_declares_vector(&field.canonical_type)))
}

fn type_declares_vector(ty: &vos::CanonicalType) -> bool {
    match ty {
        vos::CanonicalType::Named(path) => path.last().is_some_and(|name| name == "vector"),
        vos::CanonicalType::Reference(inner) | vos::CanonicalType::Optional(inner) | vos::CanonicalType::List(inner) => {
            type_declares_vector(inner)
        }
        vos::CanonicalType::Generic { path, arguments } => {
            path.last().is_some_and(|name| name == "vector")
                || arguments.iter().any(|argument| match argument {
                    vos::CanonicalTypeArgument::Type(inner) => type_declares_vector(inner),
                    vos::CanonicalTypeArgument::Literal(_) => false,
                })
        }
    }
}

enum SessionStore {
    Memory(Iris),
    Yydb(YydbSource),
    Sqlite(SqliteSource),
    /// A PostgreSQL session keeps its open transaction, if any, on this
    /// connection so statements inside `transaction` share one backend transaction.
    Postgres {
        db: PostgresSource,
        transaction: Mutex<Option<PostgresTransaction>>,
    },
    /// MySQL counterpart of [`SessionStore::Postgres`].
    Mysql {
        db: MysqlSource,
        transaction: Mutex<Option<MysqlTransaction>>,
    },
}

impl SessionStore {
    fn capabilities(&self) -> CapabilitySet {
        match self {
            Self::Memory(_) => CapabilitySet::reference_full(),
            Self::Yydb(_) => YydbSource::capabilities(),
            Self::Sqlite(_) => SqliteSource::capabilities(),
            Self::Postgres { .. } => PostgresSource::capabilities(),
            Self::Mysql { .. } => MysqlSource::capabilities(),
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
            Self::Postgres { db, transaction } => {
                let plan = planner.plan_source(source).map_err(from_iris_error)?;
                let mut guard = transaction.lock().map_err(|_| lock_failed())?;
                match guard.as_mut() {
                    Some(txn) => txn.execute_plan(&plan).map_err(from_postgres_error),
                    None => db.execute_plan(&plan).map_err(from_postgres_error),
                }
            }
            Self::Mysql { db, transaction } => {
                let plan = planner.plan_source(source).map_err(from_iris_error)?;
                let mut guard = transaction.lock().map_err(|_| lock_failed())?;
                match guard.as_mut() {
                    Some(txn) => txn.execute_plan(&plan).map_err(from_mysql_error),
                    None => db.execute_plan(&plan).map_err(from_mysql_error),
                }
            }
        }
    }

    fn execute_unit(&self, planner: &Planner, source: &str) -> Result<(), ErrorEnvelope> {
        match self {
            Self::Memory(iris) => iris.session().execute(source).map_err(from_iris_error),
            Self::Yydb(db) => db.execute(source).map_err(from_yydb_error),
            _ => self.query(planner, source).map(|_| ()),
        }
    }

    fn begin(&self) -> Result<(), ErrorEnvelope> {
        match self {
            Self::Memory(_) => Err(transaction_unsupported("memory")),
            Self::Yydb(db) => db.begin().map_err(from_yydb_error),
            Self::Sqlite(db) => db.begin().map_err(from_sqlite_error),
            Self::Postgres { db, transaction } => {
                let mut guard = transaction.lock().map_err(|_| lock_failed())?;
                if guard.is_some() {
                    return Err(transaction_already_open());
                }
                *guard = Some(db.begin_transaction().map_err(from_postgres_error)?);
                Ok(())
            }
            Self::Mysql { db, transaction } => {
                let mut guard = transaction.lock().map_err(|_| lock_failed())?;
                if guard.is_some() {
                    return Err(transaction_already_open());
                }
                *guard = Some(db.begin_transaction().map_err(from_mysql_error)?);
                Ok(())
            }
        }
    }

    fn commit(&self) -> Result<(), ErrorEnvelope> {
        match self {
            Self::Memory(_) => Err(transaction_unsupported("memory")),
            Self::Yydb(db) => db.commit().map_err(from_yydb_error),
            Self::Sqlite(db) => db.commit().map_err(from_sqlite_error),
            Self::Postgres { transaction, .. } => {
                let mut guard = transaction.lock().map_err(|_| lock_failed())?;
                match guard.take() {
                    Some(mut txn) => txn.commit().map_err(from_postgres_error),
                    None => Err(transaction_not_open()),
                }
            }
            Self::Mysql { transaction, .. } => {
                let mut guard = transaction.lock().map_err(|_| lock_failed())?;
                match guard.take() {
                    Some(mut txn) => txn.commit().map_err(from_mysql_error),
                    None => Err(transaction_not_open()),
                }
            }
        }
    }

    fn rollback(&self) -> Result<(), ErrorEnvelope> {
        match self {
            Self::Memory(_) => Err(transaction_unsupported("memory")),
            Self::Yydb(db) => db.rollback().map_err(from_yydb_error),
            Self::Sqlite(db) => db.rollback().map_err(from_sqlite_error),
            Self::Postgres { transaction, .. } => {
                let mut guard = transaction.lock().map_err(|_| lock_failed())?;
                match guard.take() {
                    Some(mut txn) => txn.rollback().map_err(from_postgres_error),
                    None => Err(transaction_not_open()),
                }
            }
            Self::Mysql { transaction, .. } => {
                let mut guard = transaction.lock().map_err(|_| lock_failed())?;
                match guard.take() {
                    Some(mut txn) => txn.rollback().map_err(from_mysql_error),
                    None => Err(transaction_not_open()),
                }
            }
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
        std::fs::create_dir_all(parent).map_err(|err| ErrorEnvelope::internal(DIAG_EXEC_FAILED, err.to_string()))?;
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
        Ok(Self::new_store(SessionStore::Postgres { db, transaction: Mutex::new(None) }, None))
    }

    /// MySQL profile over a real server URL.
    pub fn open_mysql(url: &str) -> Result<Self, ErrorEnvelope> {
        let db = MysqlSource::connect(url).map_err(from_mysql_error)?;
        Ok(Self::new_store(SessionStore::Mysql { db, transaction: Mutex::new(None) }, None))
    }

    /// Project profile resolved from an Iris config and datasource name.
    pub fn open_project(config_path: &str, source: &str) -> Result<Self, ErrorEnvelope> {
        let (project_dir, project) = load_project(Path::new(config_path)).map_err(|err| ErrorEnvelope::internal(DIAG_EXEC_FAILED, err))?;
        let ds = project.datasource(source).map_err(|err| ErrorEnvelope::internal(DIAG_EXEC_FAILED, err.to_string()))?;
        let (store, session_ddl_revision) = match ds.kind {
            DatasourceKind::Yydb => {
                let endpoint = expand_endpoint(ds, source).map_err(|err| ErrorEnvelope::internal(DIAG_EXEC_FAILED, err))?;
                let db = open_yydb_endpoint(&project_dir, &endpoint)?;
                let schema = read_schema(&project_dir, &project).map_err(|err| ErrorEnvelope::internal(DIAG_EXEC_FAILED, err))?;
                db.ensure_schema(&schema).map_err(from_yydb_error)?;
                let revision = db.schema_handshake().map_err(from_yydb_error)?.ddl_revision;
                (SessionStore::Yydb(db), Some(revision))
            }
            DatasourceKind::Sqlite => {
                let endpoint = expand_endpoint(ds, source).map_err(|err| ErrorEnvelope::internal(DIAG_EXEC_FAILED, err))?;
                let path = resolve_path(&project_dir, &endpoint);
                (SessionStore::Sqlite(SqliteSource::open(path).map_err(from_sqlite_error)?), None)
            }
            DatasourceKind::Postgres => {
                let url = expand_endpoint(ds, source).map_err(|err| ErrorEnvelope::internal(DIAG_EXEC_FAILED, err))?;
                let db = PostgresSource::connect(&url).map_err(from_postgres_error)?;
                (SessionStore::Postgres { db, transaction: Mutex::new(None) }, None)
            }
            DatasourceKind::Mysql => {
                let url = expand_endpoint(ds, source).map_err(|err| ErrorEnvelope::internal(DIAG_EXEC_FAILED, err))?;
                let db = MysqlSource::connect(&url).map_err(from_mysql_error)?;
                (SessionStore::Mysql { db, transaction: Mutex::new(None) }, None)
            }
            other => {
                return Err(ErrorEnvelope::invalid_argument(
                    DIAG_OPERATION_INVALID,
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
    /// foreign servers get their DDL from migrations instead. SQL backends
    /// also reject a schema that declares `vector` fields while the client is
    /// built, because vector search is a yydb capability in `/v0`.
    pub fn open_dsn(dsn: &str, schema: Option<&str>) -> Result<Self, ErrorEnvelope> {
        let text = dsn.trim();
        if text.is_empty() {
            return Err(ErrorEnvelope::new(CODE_INVALID_ARGUMENT, "`dsn` must be a non-empty string"));
        }
        let (session, managed) = if text == "memory:" || text == "memory" {
            (Self::open_memory(), false)
        }
        else if let Some(path) = text.strip_prefix("sqlite:") {
            // SQLite is a SQL backend: it cannot represent a `vector` field, so
            // a schema that declares one must fail here rather than in the
            // adapter's push layer.
            reject_unsupported_schema("SQLite", schema)?;
            (Self::open_sqlite(if path.is_empty() { ":memory:" } else { path })?, true)
        }
        else if let Some(path) = text.strip_prefix("file:") {
            if path.is_empty() {
                return Err(ErrorEnvelope::new(CODE_INVALID_ARGUMENT, "`file:` DSN requires a path"));
            }
            (Self::open_yydb(path)?, true)
        }
        else if text.starts_with("mysql://") {
            reject_unsupported_schema("MySQL", schema)?;
            (Self::open_mysql(text)?, false)
        }
        else if text.starts_with("postgres://") || text.starts_with("postgresql://") {
            reject_unsupported_schema("PostgreSQL", schema)?;
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
            Some(json) => bind::bind_parameters_typed(source, json).map_err(|err| ErrorEnvelope::invalid_argument(DIAG_PARAM_ENCODING, err)),
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
            Err(err) => return ExecuteOutcome::failure(ErrorEnvelope::invalid_argument(DIAG_PARAM_ENCODING, err)),
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
            Err(err) => return ExecuteOutcome::failure(ErrorEnvelope::invalid_argument(DIAG_PARAM_ENCODING, err)),
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
            Err(err) => return ExecuteOutcome::failure(ErrorEnvelope::invalid_argument(DIAG_OPERATION_INVALID, err)),
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
            _ => Err(ErrorEnvelope::invalid_argument(
                DIAG_MANAGED_PUSH_UNSUPPORTED,
                "managed_push is only supported on yydb and sqlite sessions",
            )),
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
            _ => Err(ErrorEnvelope::invalid_argument(DIAG_OPERATION_INVALID, "sqlite identity requires a sqlite session")),
        }
    }

    /// SQLite engine source id for the active sqlite session.
    pub fn sqlite_source_id(&self) -> Result<String, ErrorEnvelope> {
        if self.closed {
            return Err(closed());
        }
        match &self.store {
            Some(SessionStore::Sqlite(db)) => db.sqlite_source_id().map_err(from_sqlite_error),
            _ => Err(ErrorEnvelope::invalid_argument(DIAG_OPERATION_INVALID, "sqlite identity requires a sqlite session")),
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
    fn author_code_table_is_exactly_the_kind_table() {
        let mut kinds = std::collections::BTreeSet::new();
        for code in AUTHOR_CODES {
            let kind = kind_for(code);
            assert!(*code == CODE_INTERNAL || kind != "internal", "`{code}` must have an explicit kind instead of the fallback");
            assert!(kinds.insert(kind), "`{code}` duplicates a semantic kind");
        }
        assert_eq!(AUTHOR_CODES.len(), 8, "the author code table must hold exactly eight codes");
        assert_eq!(kinds.len(), 8, "every author code must own a distinct kind");
        for code in AUTHOR_CODES {
            assert_eq!(ErrorEnvelope::new(*code, "probe").code, *code);
        }
    }

    const VECTOR_SCHEMA: &str = r#"
table Doc {
    @@doc_id: utf8,
    embedding: vector<3>,
}
"#;

    #[test]
    fn sql_backends_reject_vector_schemas_when_the_client_is_built() {
        for (label, dsn) in [
            ("sqlite", "sqlite::memory:"),
            ("mysql", "mysql://iris@127.0.0.1:1"),
            ("postgres", "postgres://iris:@127.0.0.1:1/yyds"),
        ] {
            let err = match HostSession::open_dsn(dsn, Some(VECTOR_SCHEMA)) {
                Ok(_) => panic!("{label} accepted a schema that declares `vector`"),
                Err(err) => err,
            };
            assert_eq!(err.code, CODE_SCHEMA_MISMATCH, "{label}: {err:?}");
            assert_eq!(err.kind, "schema_mismatch", "{label}: {err:?}");
            assert_eq!(err.diagnostic_code.as_deref(), Some(DIAG_SCHEMA_VECTOR_UNSUPPORTED), "{label}: {err:?}");
            assert!(!AUTHOR_CODES.contains(&DIAG_SCHEMA_VECTOR_UNSUPPORTED), "the diagnostic stays out of the author code table");
        }
    }

    #[test]
    fn sql_backends_only_reject_schemas_that_declare_vector() {
        assert!(!schema_declares_vector(USER_SCHEMA).expect("project user schema"));
        assert!(schema_declares_vector(VECTOR_SCHEMA).expect("project vector schema"));
        assert!(!schema_declares_vector("").expect("empty schema"));
        assert!(reject_unsupported_schema("MySQL", Some(USER_SCHEMA)).is_ok());
        assert!(reject_unsupported_schema("MySQL", None).is_ok());
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
        assert_eq!(rows.error_code().as_deref(), Some(CODE_INTERNAL));

        let write = session.execute(r#"User::insert({ user_id: "u1", user_name: "alice", active: true })"#, None);
        assert!(!write.ok);
        assert_eq!(write.error_code().as_deref(), Some(CODE_INVALID_ARGUMENT));
        assert_eq!(write.error_diagnostic_code().as_deref(), Some("IRIS-PLAN-REJECTED"));

        let txn = session.begin();
        assert!(!txn.ok);
        assert_eq!(txn.error_code().as_deref(), Some(CODE_TRANSACTION_FAILED));
        assert_eq!(txn.error_diagnostic_code().as_deref(), Some(DIAG_TRANSACTION_UNSUPPORTED));
    }

    #[test]
    fn bind_and_operation_errors_keep_codes() {
        let session = HostSession::open_memory();
        let bound = session.query("User.filter(x => x.user_id == $id).collect()", Some("{not json"));
        assert!(!bound.ok);
        assert_eq!(bound.error_code().as_deref(), Some(CODE_INVALID_ARGUMENT));
        assert_eq!(bound.error_diagnostic_code().as_deref(), Some(DIAG_PARAM_ENCODING));

        let operation = session.execute_operation("{");
        assert!(!operation.ok);
        assert_eq!(operation.error_code().as_deref(), Some(CODE_INVALID_ARGUMENT));
        assert_eq!(operation.error_diagnostic_code().as_deref(), Some(DIAG_OPERATION_INVALID));
    }

    #[test]
    fn closed_session_reports_structured_code() {
        let mut session = HostSession::open_sqlite(":memory:").expect("open sqlite");
        session.close();
        let outcome = session.query("User.collect()", None);
        assert!(!outcome.ok);
        assert_eq!(outcome.error_code().as_deref(), Some(CODE_INVALID_ARGUMENT));
        assert_eq!(outcome.error_diagnostic_code().as_deref(), Some(DIAG_SESSION_CLOSED));
    }
}
