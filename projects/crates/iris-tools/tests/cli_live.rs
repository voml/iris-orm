//! CLI live bindings for Postgres / MySQL / Redis (env-gated).

use std::process::Command;

use iris::{DatasourceConfig, DatasourceKind, IrisProject, TruthMode};

fn iris_bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_iris"));
    cmd.env_remove("IRIS_TEST_POSTGRES_URL");
    cmd.env_remove("IRIS_TEST_MYSQL_URL");
    cmd.env_remove("IRIS_TEST_REDIS_URL");
    cmd
}

fn temp_dir(prefix: &str) -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("{prefix}-{stamp}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_schema(dir: &std::path::Path, table: &str) {
    std::fs::write(
        dir.join("app.iris"),
        format!(
            r#"
table {table} {{
    @@id: utf8,
    @name: utf8,
    active: bool,
}}
"#
        ),
    )
    .unwrap();
}

#[test]
fn redis_migrate_is_refused_without_sql_surface() {
    let dir = temp_dir("iris-cli-redis-refuse");
    write_schema(&dir, "CliRedis");
    let mut project = IrisProject::with_schema("app.iris");
    project.datasources.insert(
        "cache".into(),
        DatasourceConfig {
            kind: DatasourceKind::Redis,
            mode: TruthMode::AdoptExisting,
            path: None,
            // Expandable template -- migrate must refuse before connecting.
            url: Some("$IRIS_CLI_REDIS_URL_UNUSED".into()),
        },
    );
    let config = dir.join("iris.von");
    project.save(&config).unwrap();
    let cfg = config.to_str().unwrap();

    let out = iris_bin()
        .args(["--config", cfg, "migrate", "plan", "--source", "cache"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(
        err.contains("keyspace") || err.contains("not supported") || err.contains("Redis"),
        "{err}"
    );
    assert!(!err.to_ascii_lowercase().contains("create table"));

    let _ = std::fs::remove_dir_all(dir);
}

fn live_relational_round_trip(kind: DatasourceKind, env_key: &str, source: &str, table: &str) {
    let Some(url) = std::env::var(env_key).ok().filter(|s| !s.is_empty()) else {
        eprintln!("skip: set {env_key} for CLI live {source}");
        return;
    };

    let dir = temp_dir(&format!("iris-cli-{source}"));
    write_schema(&dir, table);
    // Isolate leftover tables from prior runs via unique table name.
    let mut project = IrisProject::with_schema("app.iris");
    project.datasources.insert(
        source.into(),
        DatasourceConfig {
            kind,
            mode: TruthMode::ManagedPush,
            path: None,
            url: Some("$IRIS_CLI_LIVE_URL".into()),
        },
    );
    let config = dir.join("iris.von");
    project.save(&config).unwrap();
    let cfg = config.to_str().unwrap();

    let mut bin = Command::new(env!("CARGO_BIN_EXE_iris"));
    bin.env("IRIS_CLI_LIVE_URL", &url);

    let out = bin
        .args(["--config", cfg, "inspect", "--source", source])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);

    let plan_path = dir.join("migrations").join(format!("{source}-plan.von"));
    let out = Command::new(env!("CARGO_BIN_EXE_iris"))
        .env("IRIS_CLI_LIVE_URL", &url)
        .args([
            "--config",
            cfg,
            "migrate",
            "plan",
            "--source",
            source,
            "--out",
            plan_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);

    let out = Command::new(env!("CARGO_BIN_EXE_iris"))
        .env("IRIS_CLI_LIVE_URL", &url)
        .args([
            "--config",
            cfg,
            "migrate",
            "apply",
            "--source",
            source,
            "--plan",
            plan_path.to_str().unwrap(),
            "--yes",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);

    let out = Command::new(env!("CARGO_BIN_EXE_iris"))
        .env("IRIS_CLI_LIVE_URL", &url)
        .args(["--config", cfg, "migrate", "verify", "--source", source])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    assert!(String::from_utf8_lossy(&out.stdout).contains("verify ok"));

    let mapping = dir.join("mappings").join(format!("{source}.von"));
    let out = Command::new(env!("CARGO_BIN_EXE_iris"))
        .env("IRIS_CLI_LIVE_URL", &url)
        .args([
            "--config",
            cfg,
            "adopt",
            "plan",
            "--source",
            source,
            "--out",
            mapping.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let mapping_text = std::fs::read_to_string(&mapping).unwrap();
    assert!(mapping_text.contains(table));
    assert!(!mapping_text.to_ascii_lowercase().contains("create table"));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn live_postgres_cli_round_trip() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        % 1_000_000;
    live_relational_round_trip(
        DatasourceKind::Postgres,
        "IRIS_TEST_POSTGRES_URL",
        "pg",
        &format!("IrisCliPg{stamp}"),
    );
}

#[test]
fn live_mysql_cli_round_trip() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        % 1_000_000;
    live_relational_round_trip(
        DatasourceKind::Mysql,
        "IRIS_TEST_MYSQL_URL",
        "mysql",
        &format!("IrisCliMy{stamp}"),
    );
}

#[test]
fn live_redis_inspect_and_adopt_plan() {
    let Some(url) = std::env::var("IRIS_TEST_REDIS_URL")
        .ok()
        .filter(|s| !s.is_empty())
    else {
        eprintln!("skip: set IRIS_TEST_REDIS_URL for CLI live redis");
        return;
    };

    let dir = temp_dir("iris-cli-redis");
    write_schema(&dir, "CliCache");
    let mut project = IrisProject::with_schema("app.iris");
    project.datasources.insert(
        "cache".into(),
        DatasourceConfig {
            kind: DatasourceKind::Redis,
            mode: TruthMode::AdoptExisting,
            path: None,
            url: Some("$IRIS_CLI_LIVE_URL".into()),
        },
    );
    let config = dir.join("iris.von");
    project.save(&config).unwrap();
    let cfg = config.to_str().unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_iris"))
        .env("IRIS_CLI_LIVE_URL", &url)
        .args(["--config", cfg, "inspect", "--source", "cache"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("redis"));
    assert!(stdout.contains("connectivity=ok"));

    let mapping = dir.join("mappings").join("cache.von");
    let out = Command::new(env!("CARGO_BIN_EXE_iris"))
        .env("IRIS_CLI_LIVE_URL", &url)
        .args([
            "--config",
            cfg,
            "adopt",
            "plan",
            "--source",
            "cache",
            "--out",
            mapping.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let text = std::fs::read_to_string(&mapping).unwrap();
    assert!(text.contains("CliCache"));
    assert!(text.contains("iris:clicache:"));
    assert!(!text.to_ascii_lowercase().contains("create table"));

    let out = Command::new(env!("CARGO_BIN_EXE_iris"))
        .env("IRIS_CLI_LIVE_URL", &url)
        .args(["--config", cfg, "inspect", "--source", "cache"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    assert!(String::from_utf8_lossy(&out.stdout).contains("CliCache"));

    let _ = std::fs::remove_dir_all(dir);
}
