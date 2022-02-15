//! Stateful Iris sessions for Node hosts (reference + foreign adapters).

use std::path::Path;

use crate::bind;
use crate::operation;
use iris::{CapabilitySet, DatasourceKind, Iris, Planner, ReferenceStore, Row, resolve_path};
use iris_adapter_mysql::MysqlSource;
use iris_adapter_postgres::PostgresSource;
use iris_adapter_sqlite::SqliteSource;
use iris_tools::project::{expand_endpoint, load_project};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use serde_json::{Map, Value as JsonValue, json};

fn rows_to_json(rows: Vec<Row>) -> Vec<JsonValue> {
    rows.into_iter()
        .map(|row| {
            let mut obj = Map::new();
            for (key, value) in row {
                obj.insert(key, value_to_json(&value));
            }
            JsonValue::Object(obj)
        })
        .collect()
}

fn value_to_json(value: &iris::Value) -> JsonValue {
    match value {
        iris::Value::Null => JsonValue::Null,
        iris::Value::Bool(b) => json!(b),
        iris::Value::Int(i) => json!(i),
        iris::Value::Str(s) => json!(s),
    }
}

fn ok_result(rows: Vec<Row>) -> ExecuteResult {
    ExecuteResult {
        ok: true,
        rows_json: JsonValue::Array(rows_to_json(rows)).to_string(),
        error: None,
    }
}

fn err_result(message: String) -> ExecuteResult {
    ExecuteResult {
        ok: false,
        rows_json: "[]".into(),
        error: Some(message),
    }
}

enum SessionStore {
    Memory(Iris),
    Sqlite(SqliteSource),
    Postgres(PostgresSource),
    Mysql(MysqlSource),
}

impl SessionStore {
    fn capabilities(&self) -> CapabilitySet {
        match self {
            Self::Memory(_) => CapabilitySet::reference_full(),
            Self::Sqlite(_) => SqliteSource::capabilities(),
            Self::Postgres(_) => PostgresSource::capabilities(),
            Self::Mysql(_) => MysqlSource::capabilities(),
        }
    }

    fn execute(&self, planner: &Planner, source: &str) -> std::result::Result<Vec<Row>, String> {
        match self {
            Self::Memory(iris) => iris
                .session()
                .execute_vos(source)
                .map_err(|err| err.to_string()),
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
}

impl MemorySession {
    fn new_store(store: SessionStore) -> Self {
        let planner = Planner::new(store.capabilities());
        Self {
            store,
            planner,
            closed: false,
        }
    }

    pub(crate) fn open_memory() -> Self {
        Self::new_store(SessionStore::Memory(Iris::new(
            CapabilitySet::reference_full(),
            ReferenceStore::new(),
        )))
    }

    pub(crate) fn open_sqlite(path: String) -> Result<Self> {
        let db = SqliteSource::open(path).map_err(|err| Error::from_reason(err.to_string()))?;
        Ok(Self::new_store(SessionStore::Sqlite(db)))
    }

    pub(crate) fn open_postgres(url: String) -> Result<Self> {
        let db =
            PostgresSource::connect(&url).map_err(|err| Error::from_reason(err.to_string()))?;
        Ok(Self::new_store(SessionStore::Postgres(db)))
    }

    pub(crate) fn open_mysql(url: String) -> Result<Self> {
        let db = MysqlSource::connect(&url).map_err(|err| Error::from_reason(err.to_string()))?;
        Ok(Self::new_store(SessionStore::Mysql(db)))
    }

    pub(crate) fn open_project(config_path: String, source: String) -> Result<Self> {
        let (project_dir, project) =
            load_project(Path::new(&config_path)).map_err(|err| Error::from_reason(err))?;
        let ds = project
            .datasource(&source)
            .map_err(|err| Error::from_reason(err.to_string()))?;
        let store = match ds.kind {
            DatasourceKind::Sqlite => {
                let path = resolve_path(
                    &project_dir,
                    &expand_endpoint(ds, &source).map_err(|err| Error::from_reason(err))?,
                );
                SessionStore::Sqlite(
                    SqliteSource::open(path).map_err(|err| Error::from_reason(err.to_string()))?,
                )
            }
            DatasourceKind::Postgres => {
                let url = expand_endpoint(ds, &source).map_err(|err| Error::from_reason(err))?;
                SessionStore::Postgres(
                    PostgresSource::connect(&url)
                        .map_err(|err| Error::from_reason(err.to_string()))?,
                )
            }
            DatasourceKind::Mysql => {
                let url = expand_endpoint(ds, &source).map_err(|err| Error::from_reason(err))?;
                SessionStore::Mysql(
                    MysqlSource::connect(&url)
                        .map_err(|err| Error::from_reason(err.to_string()))?,
                )
            }
            other => {
                return Err(Error::from_reason(format!(
                    "open_project_session does not support {:?} yet (use sqlite/postgres/mysql)",
                    other
                )));
            }
        };
        Ok(Self::new_store(store))
    }
}

#[napi]
impl MemorySession {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self::open_memory()
    }

    #[napi]
    pub fn execute_vos(
        &self,
        source: String,
        parameters_json: Option<String>,
    ) -> Result<ExecuteResult> {
        if self.closed {
            return Err(Error::from_reason("session closed"));
        }
        let source = match parameters_json {
            Some(json) => bind::bind_parameters(&source, &json).map_err(Error::from_reason)?,
            None => source,
        };
        match self.store.execute(&self.planner, &source) {
            Ok(rows) => Ok(ok_result(rows)),
            Err(err) => Ok(err_result(err)),
        }
    }

    /// Execute a structured Iris operation JSON payload (generated client ABI).
    #[napi(js_name = executeOperation)]
    pub fn execute_operation(&self, operation_json: String) -> Result<ExecuteResult> {
        if self.closed {
            return Err(Error::from_reason("session closed"));
        }
        let source = operation::encode_operation_json(&operation_json)
            .map_err(|err| Error::from_reason(err))?;
        match self.store.execute(&self.planner, &source) {
            Ok(rows) => Ok(ok_result(rows)),
            Err(err) => Ok(err_result(err)),
        }
    }

    /// Apply managed-push schema to a SQLite session (`:memory:` or file path).
    #[napi]
    pub fn managed_push(&self, schema: String) -> Result<()> {
        if self.closed {
            return Err(Error::from_reason("session closed"));
        }
        match &self.store {
            SessionStore::Sqlite(db) => {
                db.managed_push(&schema)
                    .map_err(|err| Error::from_reason(err.to_string()))?;
                Ok(())
            }
            _ => Err(Error::from_reason(
                "managed_push is only supported on sqlite sessions",
            )),
        }
    }

    #[napi]
    pub fn close(&mut self) {
        self.closed = true;
    }
}
