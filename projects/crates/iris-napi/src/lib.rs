//! Node N-API surface for Iris ORM.
//!
//! Thin binding over the Rust semantic core — no parallel TS parser.

#![deny(clippy::all)]

mod bind;
mod operation;
mod session;

use std::path::Path;

use iris_generator::GenerationModel;
use iris_tools::{migrate_plan, migrate_run, project};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use session::MemorySession;

/// Library version (matches `iris::version()` / Cargo package version).
#[napi]
pub fn iris_version() -> String {
    iris::version().to_string()
}

/// Result of validating a VOS / `.iris` schema source via the Rust core.
#[napi(object)]
pub struct CheckSourceResult {
    pub ok: bool,
    pub table_count: u32,
    pub schema_fingerprint: String,
    pub generator_version: String,
    pub error: Option<String>,
}

/// Parse and validate schema source (same semantics as `iris-tools check`).
#[napi]
pub fn check_source(source: String) -> Result<CheckSourceResult> {
    match GenerationModel::from_vos_schema(&source) {
        Ok(model) => Ok(CheckSourceResult {
            ok: true,
            table_count: u32::try_from(model.tables.len()).unwrap_or(u32::MAX),
            schema_fingerprint: model.schema_fingerprint,
            generator_version: model.generator_version,
            error: None,
        }),
        Err(err) => Ok(CheckSourceResult {
            ok: false,
            table_count: 0,
            schema_fingerprint: String::new(),
            generator_version: String::new(),
            error: Some(err.to_string()),
        }),
    }
}

/// Read-only schema introspection JSON (`GenerationModel` shape).
#[napi]
pub fn introspect_schema(source: String) -> String {
    iris_wasm::introspect_schema_json(&source)
}

/// Loaded on-disk project metadata.
#[napi(object)]
pub struct LoadProjectResult {
    pub root: String,
    pub config: String,
    pub schema_glob: String,
    pub generate_out: String,
    pub generate_target: String,
}

/// Load `iris.von` from a config path.
#[napi]
pub fn load_project(config_path: String) -> Result<LoadProjectResult> {
    let (root, project) =
        project::load_project(Path::new(&config_path)).map_err(|e| Error::from_reason(e))?;
    Ok(LoadProjectResult {
        root: root.display().to_string(),
        config: config_path,
        schema_glob: project.schema,
        generate_out: project.generate.out,
        generate_target: project.generate.target,
    })
}

/// Read merged schema text for a loaded project.
#[napi]
pub fn read_schema(project_root: String, schema_glob: String) -> Result<String> {
    let project = iris::IrisProject {
        schema: schema_glob,
        ..Default::default()
    };
    project::read_schema(Path::new(&project_root), &project).map_err(|e| Error::from_reason(e))
}

/// Codegen output summary.
#[napi(object)]
pub struct GenerateResult {
    pub ok: bool,
    pub output_path: String,
    pub schema_fingerprint: String,
    pub files: Vec<String>,
    pub error: Option<String>,
}

/// Generate client bindings (`rust` | `typescript`) from merged schema source.
#[napi]
pub fn generate(source: String, target: String, out_dir: String) -> Result<GenerateResult> {
    match iris_generator::generate_from_source(&source, &target, Path::new(&out_dir)) {
        Ok((model, paths)) => {
            let output_path = match target.as_str() {
                "rust" => Path::new(&out_dir)
                    .join("generated/iris/rust")
                    .display()
                    .to_string(),
                "typescript" | "ts" => Path::new(&out_dir)
                    .join("generated/iris/typescript")
                    .display()
                    .to_string(),
                _ => out_dir.clone(),
            };
            Ok(GenerateResult {
                ok: true,
                output_path,
                schema_fingerprint: model.schema_fingerprint,
                files: paths
                    .into_iter()
                    .map(|path| path.display().to_string())
                    .collect(),
                error: None,
            })
        }
        Err(err) => Ok(GenerateResult {
            ok: false,
            output_path: String::new(),
            schema_fingerprint: String::new(),
            files: vec![],
            error: Some(err.to_string()),
        }),
    }
}

/// Migration plan summary.
#[napi(object)]
pub struct MigratePlanResult {
    pub ok: bool,
    pub plan_path: String,
    pub error: Option<String>,
}

/// Plan a managed-push migration (same as `iris-tools migrate plan`).
#[napi]
pub fn migrate_plan_cmd(
    config_path: String,
    source: String,
    out_dir: Option<String>,
) -> Result<MigratePlanResult> {
    let out = out_dir.as_deref().map(Path::new);
    match migrate_plan(Path::new(&config_path), &source, out) {
        Ok(path) => Ok(MigratePlanResult {
            ok: true,
            plan_path: path.display().to_string(),
            error: None,
        }),
        Err(err) => Ok(MigratePlanResult {
            ok: false,
            plan_path: String::new(),
            error: Some(err),
        }),
    }
}

/// Full managed-push run summary (plan → apply → verify).
#[napi(object)]
pub struct MigrateRunResult {
    pub ok: bool,
    pub plan_path: String,
    pub plan_only: bool,
    pub created_tables: Vec<String>,
    pub error: Option<String>,
}

/// Plan → apply → verify (same as `iris-tools migrate run` / library `migrate_run`).
#[napi]
pub fn migrate_run_cmd(
    config_path: String,
    source: String,
    plan_out: Option<String>,
    plan_only: bool,
) -> Result<MigrateRunResult> {
    let config = Path::new(&config_path);
    let plan_path = plan_out
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            config
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("migrations")
                .join(format!("{source}-plan.von"))
        });
    match migrate_run(config, &source, &plan_path, plan_only) {
        Ok(None) => Ok(MigrateRunResult {
            ok: true,
            plan_path: plan_path.display().to_string(),
            plan_only: true,
            created_tables: vec![],
            error: None,
        }),
        Ok(Some(report)) => Ok(MigrateRunResult {
            ok: true,
            plan_path: plan_path.display().to_string(),
            plan_only: false,
            created_tables: report.created_tables,
            error: None,
        }),
        Err(err) => Ok(MigrateRunResult {
            ok: false,
            plan_path: plan_path.display().to_string(),
            plan_only: plan_only,
            created_tables: vec![],
            error: Some(err),
        }),
    }
}

/// Open an in-memory reference session.
#[napi]
pub fn open_memory_session() -> Result<MemorySession> {
    Ok(MemorySession::open_memory())
}

/// Session open options (unified N-API entry).
#[napi(object)]
pub struct OpenSessionOptions {
    /// `memory` | `sqlite` | `postgres` | `mysql` | `project`
    pub profile: Option<String>,
    pub sqlite_path: Option<String>,
    pub postgres_url: Option<String>,
    pub mysql_url: Option<String>,
    pub project_config: Option<String>,
    pub datasource: Option<String>,
}

/// Open a session for the requested adapter profile.
#[napi]
pub fn open_session(options: Option<OpenSessionOptions>) -> Result<MemorySession> {
    let opts = options.unwrap_or(OpenSessionOptions {
        profile: Some("memory".into()),
        sqlite_path: None,
        postgres_url: None,
        mysql_url: None,
        project_config: None,
        datasource: None,
    });
    let profile = opts.profile.as_deref().unwrap_or("memory");
    match profile {
        "memory" | "reference" => Ok(MemorySession::open_memory()),
        "sqlite" => {
            let path = opts.sqlite_path.unwrap_or_else(|| ":memory:".into());
            MemorySession::open_sqlite(path)
        }
        "postgres" => {
            let url = opts.postgres_url.ok_or_else(|| {
                Error::from_reason("open_session(profile=postgres): postgres_url is required")
            })?;
            MemorySession::open_postgres(url)
        }
        "mysql" => {
            let url = opts.mysql_url.ok_or_else(|| {
                Error::from_reason("open_session(profile=mysql): mysql_url is required")
            })?;
            MemorySession::open_mysql(url)
        }
        "project" => {
            let config = opts.project_config.ok_or_else(|| {
                Error::from_reason("open_session(profile=project): project_config is required")
            })?;
            let source = opts.datasource.unwrap_or_else(|| "default".into());
            MemorySession::open_project(config, source)
        }
        other => Err(Error::from_reason(format!(
            "open_session: unknown profile `{other}` (use memory|sqlite|postgres|mysql|project)"
        ))),
    }
}

/// Open a SQLite session on a filesystem path or `:memory:`.
#[napi]
pub fn open_sqlite_session(path: String) -> Result<MemorySession> {
    MemorySession::open_sqlite(path)
}

/// Open a PostgreSQL session from a connection URL.
#[napi]
pub fn open_postgres_session(url: String) -> Result<MemorySession> {
    MemorySession::open_postgres(url)
}

/// Open a MySQL session from a connection URL.
#[napi]
pub fn open_mysql_session(url: String) -> Result<MemorySession> {
    MemorySession::open_mysql(url)
}

/// Open a session for a datasource declared in `iris.von`.
#[napi]
pub fn open_project_session(config_path: String, source: String) -> Result<MemorySession> {
    MemorySession::open_project(config_path, source)
}
