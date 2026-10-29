//! Stateful Iris sessions for Node hosts (reference + foreign adapters).

use std::path::Path;

use crate::operation;
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
use napi::bindgen_prelude::*;
use napi_derive::napi;
use serde_json::Value as JsonValue;

fn ok_result(rows: Vec<Row>) -> ExecuteResult {
    ExecuteResult { ok: true, rows_json: JsonValue::Array(rows_to_json(rows)).to_string(), error: None }
}

fn err_result(message: String) -> ExecuteResult {
    ExecuteResult { ok: false, rows_json: "[]".into(), error: Some(message) }
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

    fn query(&self, planner: &Planner, source: &str) -> std::result::Result<Vec<Row>, String> {
        match self {
            Self::Memory(iris) => iris.session().query(source).map_err(|err| err.to_string()),
            Self::Yydb(db) => db.query(source).map_err(|err| err.to_string()),
            Self::Sqlite(db) => {
                let plan = planner.plan_source(source).map_err(|err| err.to_string())?;
                db.execute_plan(&plan).map_err(|err| err.to_string())
            }
            Self::Postgres(db) => {
                let plan = planner.plan_source(source).map_err(|err| err.to_string())?;
                db.execute_plan(&plan).map_err(|err| err.to_string())
            }
            Self::Mysql(db) => {
                let plan = planner.plan_source(source).map_err(|err| err.to_string())?;
                db.execute_plan(&plan).map_err(|err| err.to_string())
            }
        }
    }

    fn execute_unit(&self, planner: &Planner, source: &str) -> std::result::Result<(), String> {
        match self {
            Self::Memory(iris) => iris.session().execute(source).map_err(|err| err.to_string()),
            Self::Yydb(db) => db.execute(source).map_err(|err| err.to_string()),
            Self::Sqlite(db) => {
                let plan = planner.plan_source(source).map_err(|err| err.to_string())?;
                db.execute_plan(&plan).map_err(|err| err.to_string())?;
                Ok(())
            }
            Self::Postgres(db) => {
                let plan = planner.plan_source(source).map_err(|err| err.to_string())?;
                db.execute_plan(&plan).map_err(|err| err.to_string())?;
                Ok(())
            }
            Self::Mysql(db) => {
                let plan = planner.plan_source(source).map_err(|err| err.to_string())?;
                db.execute_plan(&plan).map_err(|err| err.to_string())?;
                Ok(())
            }
        }
    }
}

/// Execute VOS against the bound adapter.
#[napi(object)]
pub struct ExecuteResult {
    pub ok: bool,
    /// JSON array of row objects.
    pub rows_json: String,
    pub error: Option<String>,
}

/// Iris session bound to a reference or foreign adapter.
#[napi]
pub struct MemorySession {
    store: SessionStore,
    planner: Planner,
    closed: bool,
    session_ddl_revision: Option<u64>,
}

fn open_yydb_endpoint(project_dir: &Path, endpoint: &str) -> std::result::Result<YydbSource, String> {
    if endpoint == ":memory:" {
        return YydbSource::open_in_memory().map_err(|err| err.to_string());
    }
    let path = endpoint.strip_prefix("file:").unwrap_or(endpoint);
    let path = resolve_path(project_dir, path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    YydbSource::open(path).map_err(|err| err.to_string())
}

impl MemorySession {
    fn bind_parameters(&self, source: &str, parameters_json: Option<&str>) -> Result<bind::BoundParameters> {
        match parameters_json {
            Some(json) => bind::bind_parameters_typed(source, json).map_err(Error::from_reason),
            None => Ok(bind::BoundParameters::unbound(source)),
        }
    }

    /// Run a bound read pipeline: YYDB takes typed values, other adapters take text only.
    fn run_bound_query(&self, bound: &bind::BoundParameters) -> Result<ExecuteResult> {
        if let SessionStore::Yydb(db) = &self.store {
            self.ensure_yydb_session_fresh(db).map_err(Error::from_reason)?;
            return match db.query_with_params(&bound.source, &bound.value_parameters) {
                Ok(rows) => Ok(ok_result(rows)),
                Err(err) => Ok(err_result(err.to_string())),
            };
        }
        let source = bound.text_only().map_err(Error::from_reason)?;
        match self.store.query(&self.planner, source) {
            Ok(rows) => Ok(ok_result(rows)),
            Err(err) => Ok(err_result(err)),
        }
    }

    fn new_store(store: SessionStore, session_ddl_revision: Option<u64>) -> Self {
        let planner = Planner::new(store.capabilities());
        Self { store, planner, closed: false, session_ddl_revision }
    }

    fn ensure_yydb_session_fresh(&self, db: &YydbSource) -> std::result::Result<(), String> {
        if let Some(expected) = self.session_ddl_revision {
            db.check_session_ddl_revision(expected).map_err(|err| err.to_string())?;
        }
        Ok(())
    }

    pub(crate) fn open_memory() -> Self {
        Self::new_store(SessionStore::Memory(Iris::new(CapabilitySet::reference_full(), ReferenceStore::new())), None)
    }

    pub(crate) fn open_sqlite(path: String) -> Result<Self> {
        let db = if path == ":memory:" {
            SqliteSource::open_in_memory().map_err(|err| Error::from_reason(err.to_string()))?
        }
        else {
            SqliteSource::open(path).map_err(|err| Error::from_reason(err.to_string()))?
        };
        Ok(Self::new_store(SessionStore::Sqlite(db), None))
    }

    fn sqlite_source(&self) -> Result<&SqliteSource> {
        match &self.store {
            SessionStore::Sqlite(db) => Ok(db),
            _ => Err(Error::from_reason("sqlite identity requires a sqlite session")),
        }
    }

    pub(crate) fn open_postgres(url: String) -> Result<Self> {
        let db = PostgresSource::connect(&url).map_err(|err| Error::from_reason(err.to_string()))?;
        Ok(Self::new_store(SessionStore::Postgres(db), None))
    }

    pub(crate) fn open_mysql(url: String) -> Result<Self> {
        let db = MysqlSource::connect(&url).map_err(|err| Error::from_reason(err.to_string()))?;
        Ok(Self::new_store(SessionStore::Mysql(db), None))
    }

    pub(crate) fn open_project(config_path: String, source: String) -> Result<Self> {
        let (project_dir, project) = load_project(Path::new(&config_path)).map_err(|err| Error::from_reason(err))?;
        let ds = project.datasource(&source).map_err(|err| Error::from_reason(err.to_string()))?;
        let (store, session_ddl_revision) = match ds.kind {
            DatasourceKind::Yydb => {
                let endpoint = expand_endpoint(ds, &source).map_err(|err| Error::from_reason(err))?;
                let db = open_yydb_endpoint(&project_dir, &endpoint).map_err(|err| Error::from_reason(err))?;
                let schema = read_schema(&project_dir, &project).map_err(|err| Error::from_reason(err))?;
                db.ensure_schema(&schema).map_err(|err| Error::from_reason(err.to_string()))?;
                let revision = db.schema_handshake().map_err(|err| Error::from_reason(err.to_string()))?.ddl_revision;
                (SessionStore::Yydb(db), Some(revision))
            }
            DatasourceKind::Sqlite => {
                let path = resolve_path(&project_dir, &expand_endpoint(ds, &source).map_err(|err| Error::from_reason(err))?);
                (SessionStore::Sqlite(SqliteSource::open(path).map_err(|err| Error::from_reason(err.to_string()))?), None)
            }
            DatasourceKind::Postgres => {
                let url = expand_endpoint(ds, &source).map_err(|err| Error::from_reason(err))?;
                (SessionStore::Postgres(PostgresSource::connect(&url).map_err(|err| Error::from_reason(err.to_string()))?), None)
            }
            DatasourceKind::Mysql => {
                let url = expand_endpoint(ds, &source).map_err(|err| Error::from_reason(err))?;
                (SessionStore::Mysql(MysqlSource::connect(&url).map_err(|err| Error::from_reason(err.to_string()))?), None)
            }
            other => {
                return Err(Error::from_reason(format!(
                    "open_project_session does not support {:?} yet (use yydb/sqlite/postgres/mysql)",
                    other
                )));
            }
        };
        Ok(Self::new_store(store, session_ddl_revision))
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
        if self.closed {
            return Err(Error::from_reason("session closed"));
        }
        let bound = self.bind_parameters(&source, parameters_json.as_deref())?;
        self.run_bound_query(&bound)
    }

    /// Execute unit-valued / DDL-shaped VOS. Aligns with generated `db.$execute`.
    #[napi]
    pub fn execute(&self, source: String, parameters_json: Option<String>) -> Result<ExecuteResult> {
        if self.closed {
            return Err(Error::from_reason("session closed"));
        }
        let bound = self.bind_parameters(&source, parameters_json.as_deref())?;
        if let SessionStore::Yydb(db) = &self.store {
            self.ensure_yydb_session_fresh(db).map_err(Error::from_reason)?;
            return match db.execute_with_params(&bound.source, &bound.value_parameters) {
                Ok(()) => Ok(ExecuteResult { ok: true, rows_json: "[]".into(), error: None }),
                Err(err) => Ok(err_result(err.to_string())),
            };
        }
        let source = bound.text_only().map_err(Error::from_reason)?;
        match self.store.execute_unit(&self.planner, source) {
            Ok(()) => Ok(ExecuteResult { ok: true, rows_json: "[]".into(), error: None }),
            Err(err) => Ok(err_result(err)),
        }
    }

    /// Plan + execute VOS DML (legacy name).
    #[napi]
    pub fn execute_vos(&self, source: String, parameters_json: Option<String>) -> Result<ExecuteResult> {
        self.query(source, parameters_json)
    }

    /// Execute a structured Iris operation JSON payload (generated client ABI).
    #[napi(js_name = executeOperation)]
    pub fn execute_operation(&self, request_json: String) -> Result<ExecuteResult> {
        if self.closed {
            return Err(Error::from_reason("session closed"));
        }
        let bound = operation::encode_request_json(&request_json).map_err(|err| Error::from_reason(err))?;
        self.run_bound_query(&bound)
    }

    /// Apply managed-push schema to a SQLite session (`:memory:` or file path).
    #[napi]
    pub fn managed_push(&self, schema: String) -> Result<()> {
        if self.closed {
            return Err(Error::from_reason("session closed"));
        }
        match &self.store {
            SessionStore::Yydb(db) => {
                db.ensure_schema(&schema).map_err(|err| Error::from_reason(err.to_string()))?;
                Ok(())
            }
            SessionStore::Sqlite(db) => {
                db.managed_push(&schema).map_err(|err| Error::from_reason(err.to_string()))?;
                Ok(())
            }
            _ => Err(Error::from_reason("managed_push is only supported on yydb and sqlite sessions")),
        }
    }

    /// SQLite engine version for the active sqlite session.
    #[napi(js_name = sqliteEngineVersion)]
    pub fn sqlite_engine_version(&self) -> Result<String> {
        if self.closed {
            return Err(Error::from_reason("session closed"));
        }
        self.sqlite_source()?.sqlite_version().map_err(|err| Error::from_reason(err.to_string()))
    }

    /// SQLite engine source id for the active sqlite session.
    #[napi(js_name = sqliteSourceId)]
    pub fn sqlite_source_id(&self) -> Result<String> {
        if self.closed {
            return Err(Error::from_reason("session closed"));
        }
        self.sqlite_source()?.sqlite_source_id().map_err(|err| Error::from_reason(err.to_string()))
    }

    #[napi]
    pub fn close(&mut self) {
        self.closed = true;
    }
}
