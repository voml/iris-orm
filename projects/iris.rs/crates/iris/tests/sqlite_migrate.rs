//! Managed-push migrate (plan / apply / verify) through the public facade for SQLite.

#![cfg(feature = "migrate")]

use std::path::{Path, PathBuf};

use iris_types::{DatasourceConfig, DatasourceKind, IrisProject, TruthMode};

const USER_SCHEMA: &str = r#"
table User {
    @@user_id: utf8,
    @user_name: utf8,
    active: bool,
}
"#;

fn temp_dir(label: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("iris-sqlite-migrate-{label}-{stamp}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_project(dir: &Path, database: &Path) -> PathBuf {
    std::fs::create_dir_all(dir.join("schemas")).unwrap();
    std::fs::write(dir.join("schemas/user.iris"), USER_SCHEMA).unwrap();
    let mut project = IrisProject::with_schema("schemas/user.iris");
    project.datasources.insert(
        "db".into(),
        DatasourceConfig { kind: DatasourceKind::Sqlite, mode: TruthMode::ManagedPush, path: Some(database.display().to_string()), url: None },
    );
    let config = dir.join("iris.von");
    project.save(&config).unwrap();
    config
}

#[test]
fn facade_migrate_plan_apply_verify_runs_against_sqlite() {
    let dir = temp_dir("plan");
    let config = write_project(&dir, &dir.join("app.db"));

    let plan_path = iris::migrate_plan(&config, "db", None).unwrap();
    assert!(plan_path.is_file(), "migrate plan must be written to {}", plan_path.display());
    let plan_text = std::fs::read_to_string(&plan_path).unwrap();
    assert!(plan_text.contains("User"), "plan must reference the VOS table, got: {plan_text}");

    let report = iris::migrate_apply(&config, "db", &plan_path).unwrap();
    assert_eq!(report.created_tables, vec!["User".to_string()]);
    iris::migrate_verify(&config, "db").unwrap();

    let _ = std::fs::remove_dir_all(&dir);
}
