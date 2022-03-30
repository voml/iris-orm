//! Managed-push migration (plan / apply / verify) — library surface for embedders.

use std::path::{Path, PathBuf};

use iris::{
    DatasourceKind, DriftReport, LogicalMigrationPlan, TruthMode, default_migration_plan,
    resolve_path,
};
use iris_adapter_mysql::MysqlSource;
use iris_adapter_postgres::PostgresSource;
use iris_adapter_sqlite::SqliteSource;

use crate::project::{expand_endpoint, load_project, read_schema, write_file};

/// Tables created by a successful `migrate_apply`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyReport {
    /// Plan id from the logical migration plan.
    pub plan_id: String,
    /// Physical tables created.
    pub created_tables: Vec<String>,
}

fn refuse_redis_migrate(kind: DatasourceKind) -> Result<(), String> {
    if kind == DatasourceKind::Redis {
        Err(
            "Redis is a keyspace adapter — migrate plan/apply/verify is not supported; \
             use adopt plan for explicit mappings"
                .into(),
        )
    } else {
        Ok(())
    }
}

fn print_drift(source: &str, drift: &DriftReport) -> Result<(), String> {
    if drift.is_push_satisfied() {
        if !drift.extra_physical_tables.is_empty() {
            eprintln!(
                "note: source={source}: {} unrelated physical table(s) ignored for managed_push verify",
                drift.extra_physical_tables.len()
            );
        }
        Ok(())
    } else {
        Err(format!("drift detected for source={source}: {drift:?}"))
    }
}

/// Plan a managed-push migration; returns the plan path written.
pub fn migrate_plan(config: &Path, source: &str, out: Option<&Path>) -> Result<PathBuf, String> {
    let (project_dir, project) = load_project(config)?;
    let ds = project.datasource(source).map_err(|e| e.to_string())?;
    refuse_redis_migrate(ds.kind)?;
    if ds.mode != TruthMode::ManagedPush {
        return Err(format!(
            "migrate plan requires managed_push mode (got {:?})",
            ds.mode
        ));
    }
    let schema = read_schema(&project_dir, &project)?;
    let plan = match ds.kind {
        DatasourceKind::Sqlite => {
            let path = resolve_path(&project_dir, &expand_endpoint(ds, source)?);
            let db = SqliteSource::open(path).map_err(|e| e.to_string())?;
            db.plan_managed_push(&schema).map_err(|e| e.to_string())?
        }
        DatasourceKind::Postgres => {
            let url = expand_endpoint(ds, source)?;
            let db = PostgresSource::connect(&url).map_err(|e| e.to_string())?;
            db.plan_managed_push(&schema).map_err(|e| e.to_string())?
        }
        DatasourceKind::Mysql => {
            let url = expand_endpoint(ds, source)?;
            let db = MysqlSource::connect(&url).map_err(|e| e.to_string())?;
            db.plan_managed_push(&schema).map_err(|e| e.to_string())?
        }
        other => {
            return Err(format!(
                "migrate plan does not support {:?} (relational: sqlite/postgres/mysql)",
                other
            ));
        }
    };
    let out_path = out
        .map(Path::to_path_buf)
        .unwrap_or_else(|| default_migration_plan(&project_dir, source));
    let text = von::to_string_indented(&plan).map_err(|e| e.to_string())?;
    write_file(&out_path, &text)?;
    Ok(out_path)
}

/// Apply a reviewed logical migration plan.
pub fn migrate_apply(config: &Path, source: &str, plan_path: &Path) -> Result<ApplyReport, String> {
    let (project_dir, project) = load_project(config)?;
    let ds = project.datasource(source).map_err(|e| e.to_string())?;
    refuse_redis_migrate(ds.kind)?;
    let schema = read_schema(&project_dir, &project)?;
    let plan_text = std::fs::read_to_string(plan_path).map_err(|e| e.to_string())?;
    let plan: LogicalMigrationPlan = von::from_str(&plan_text).map_err(|e| e.to_string())?;
    let (plan_id, created) = match ds.kind {
        DatasourceKind::Sqlite => {
            let path = resolve_path(&project_dir, &expand_endpoint(ds, source)?);
            let db = SqliteSource::open(path).map_err(|e| e.to_string())?;
            let report = db
                .apply_managed_push(&plan, &schema)
                .map_err(|e| e.to_string())?;
            (report.plan_id, report.created_tables)
        }
        DatasourceKind::Postgres => {
            let url = expand_endpoint(ds, source)?;
            let db = PostgresSource::connect(&url).map_err(|e| e.to_string())?;
            let report = db
                .apply_managed_push(&plan, &schema)
                .map_err(|e| e.to_string())?;
            (report.plan_id, report.created_tables)
        }
        DatasourceKind::Mysql => {
            let url = expand_endpoint(ds, source)?;
            let db = MysqlSource::connect(&url).map_err(|e| e.to_string())?;
            let report = db
                .apply_managed_push(&plan, &schema)
                .map_err(|e| e.to_string())?;
            (report.plan_id, report.created_tables)
        }
        other => {
            return Err(format!("migrate apply does not support {:?}", other));
        }
    };
    Ok(ApplyReport {
        plan_id,
        created_tables: created,
    })
}

/// Re-inspect and fail when schema drift is detected.
pub fn migrate_verify(config: &Path, source: &str) -> Result<(), String> {
    let (project_dir, project) = load_project(config)?;
    let ds = project.datasource(source).map_err(|e| e.to_string())?;
    refuse_redis_migrate(ds.kind)?;
    let schema = read_schema(&project_dir, &project)?;
    let drift = match ds.kind {
        DatasourceKind::Sqlite => {
            let path = resolve_path(&project_dir, &expand_endpoint(ds, source)?);
            let db = SqliteSource::open(path).map_err(|e| e.to_string())?;
            db.drift(&schema).map_err(|e| e.to_string())?
        }
        DatasourceKind::Postgres => {
            let url = expand_endpoint(ds, source)?;
            let db = PostgresSource::connect(&url).map_err(|e| e.to_string())?;
            db.drift(&schema).map_err(|e| e.to_string())?
        }
        DatasourceKind::Mysql => {
            let url = expand_endpoint(ds, source)?;
            let db = MysqlSource::connect(&url).map_err(|e| e.to_string())?;
            db.drift(&schema).map_err(|e| e.to_string())?
        }
        other => {
            return Err(format!("migrate verify does not support {:?}", other));
        }
    };
    print_drift(source, &drift)
}

/// Plan → apply → verify (embedder default for offline DDL).
pub fn migrate_run(
    config: &Path,
    source: &str,
    plan_out: &Path,
    plan_only: bool,
) -> Result<Option<ApplyReport>, String> {
    let plan_path = migrate_plan(config, source, Some(plan_out))?;
    if plan_only {
        return Ok(None);
    }
    let report = migrate_apply(config, source, &plan_path)?;
    migrate_verify(config, source)?;
    Ok(Some(report))
}
