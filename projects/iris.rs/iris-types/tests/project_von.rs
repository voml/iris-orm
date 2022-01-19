//! Phase 9: iris.von project config.

use iris_types::{
    DatasourceConfig, DatasourceKind, IrisLock, IrisProject, PROJECT_FORMAT, TruthMode, expand_env,
};

#[test]
fn project_round_trips_von_and_rejects_bad_format() {
    let mut project = IrisProject::with_schema("schema/app.iris");
    project.datasources.insert(
        "main".into(),
        DatasourceConfig {
            kind: DatasourceKind::Sqlite,
            mode: TruthMode::ManagedPush,
            path: Some("$IRIS_SQLITE_PATH".into()),
            url: None,
        },
    );
    let text = project.to_von().unwrap();
    assert!(text.contains(PROJECT_FORMAT));
    assert!(text.contains("sqlite"));
    assert!(text.contains("$IRIS_SQLITE_PATH"));
    let parsed = IrisProject::parse(&text).unwrap();
    assert_eq!(parsed, project);

    let bad = text.replace(PROJECT_FORMAT, "other.project");
    assert!(IrisProject::parse(&bad).is_err());
}

#[test]
fn rejects_inline_password_urls() {
    let mut project = IrisProject::with_schema("schema/app.iris");
    project.datasources.insert(
        "db".into(),
        DatasourceConfig {
            kind: DatasourceKind::Postgres,
            mode: TruthMode::ManagedPush,
            path: None,
            url: Some("postgres://iris:secret@127.0.0.1/iris".into()),
        },
    );
    let err = project.validate().unwrap_err();
    assert!(
        err.to_string().contains("password")
            || err.to_string().contains("secret")
            || err.to_string().contains("$ENV")
    );
}

#[test]
fn expand_env_supports_dollar_and_braces() {
    unsafe {
        std::env::set_var("IRIS_TEST_A", "alpha");
        std::env::set_var("IRIS_TEST_B", "beta");
    }
    assert_eq!(
        expand_env("pre-$IRIS_TEST_A-${IRIS_TEST_B}-post").unwrap(),
        "pre-alpha-beta-post"
    );
}

#[test]
fn default_schema_omitted_from_von() {
    let mut project = IrisProject::new();
    project.datasources.insert(
        "main".into(),
        DatasourceConfig {
            kind: DatasourceKind::Mysql,
            mode: TruthMode::ManagedPush,
            path: None,
            url: Some("$MYSQL_URL".into()),
        },
    );
    let text = project.to_von().unwrap();
    assert!(!text.contains("schema:"));
    let parsed = IrisProject::parse(&text).unwrap();
    assert_eq!(parsed.schema, iris_types::DEFAULT_SCHEMA);
}

#[test]
fn default_generate_omitted_from_von() {
    let project = IrisProject::new();
    let text = project.to_von().unwrap();
    assert!(!text.contains("generate:"));
    let parsed = IrisProject::parse(&text).unwrap();
    assert_eq!(parsed.generate.out, iris_types::DEFAULT_GENERATE_DIR);
}

#[test]
fn cache_paths_anchor_at_workspace_root() {
    use iris_types::{DEFAULT_GENERATE_DIR, find_workspace_root, resolve_path};
    use std::path::Path;

    let farm_db = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../vmz-circle-farm/backends/farm-database");
    if !farm_db.join("iris.von").is_file() {
        eprintln!("skip: vmz-circle-farm not sibling");
        return;
    }
    let ws = find_workspace_root(&farm_db);
    assert!(ws.join("Cargo.toml").is_file());
    let generate_out = resolve_path(&farm_db, DEFAULT_GENERATE_DIR);
    assert!(generate_out.starts_with(ws.join(".cache/iris")));
}

#[test]
fn lock_round_trip() {
    let lock = IrisLock::new("abc", "0.1.0", "rust");
    let text = lock.to_von().unwrap();
    let parsed = IrisLock::parse(&text).unwrap();
    assert_eq!(parsed.schema_fingerprint, "abc");
}
