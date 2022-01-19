//! CLI smoke for check / generate / doctor / iris.von ops (no SQL surface).

use std::process::Command;

use iris::{DatasourceConfig, DatasourceKind, IrisProject, TruthMode};

fn iris_bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_iris"));
    cmd.env_remove("IRIS_TEST_POSTGRES_URL");
    cmd
}

#[test]
fn version_and_doctor_and_capabilities() {
    let out = iris_bin().arg("version").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("iris "));

    let out = iris_bin().arg("doctor").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("no-sql-invariant"));
    assert!(stdout.contains("IRIS-YYDS-VOS-NOT-READY") || stdout.contains("yyds readiness"));
    assert!(stdout.contains("cli live"));

    let out = iris_bin().arg("capabilities").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("redis"));
    assert!(stdout.contains("yydb"));
    assert!(!stdout.to_ascii_lowercase().contains("select "));
}

#[test]
fn check_and_generate_round_trip() {
    let dir = std::env::temp_dir().join(format!(
        "iris-cli-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    // Iris schema on-disk extension is `.iris`.
    let schema = dir.join("user.iris");
    std::fs::write(
        &schema,
        r#"
table User {
    @@user_id: utf8,
    @user_name: utf8,
    active: bool,
}
"#,
    )
    .unwrap();

    let out = iris_bin()
        .args(["check", schema.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("ok:"));
    assert!(stdout.contains("User"));

    let gen_out = dir.join(".cache").join("iris").join("generate");
    let out = iris_bin()
        .args([
            "generate",
            schema.to_str().unwrap(),
            "--out",
            gen_out.to_str().unwrap(),
            "--target",
            "rust",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let mod_rs = std::fs::read_to_string(gen_out.join("mod.rs")).unwrap();
    assert!(mod_rs.contains("pub struct User"));
    assert!(!mod_rs.contains("CREATE TABLE"));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn rejects_sql_shaped_subcommand_names_are_absent() {
    let out = iris_bin().arg("--help").output().unwrap();
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(!help.to_ascii_lowercase().contains("query --sql"));
    assert!(!help.contains("sql "));
    assert!(help.contains("inspect"));
    assert!(help.contains("adopt"));
    assert!(help.contains("topology"));
    assert!(help.contains("explain"));
    assert!(help.contains("projection"));
    assert!(help.contains("object"));
}

#[test]
fn iris_von_inspect_adopt_migrate_round_trip() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("iris-von-{stamp}"));
    std::fs::create_dir_all(&dir).unwrap();
    let schema = dir.join("app.iris");
    std::fs::write(
        &schema,
        r#"
table User {
    @@user_id: utf8,
    @user_name: utf8,
    active: bool,
}
"#,
    )
    .unwrap();
    let db_path = dir.join("app.yysqlite");
    let config = dir.join("iris.von");
    // Absolute path avoids env coupling; no credentials.
    let db_path_von = db_path.to_str().unwrap().replace('\\', "/");
    let mut project = IrisProject::with_schema("app.iris");
    project.datasources.insert(
        "main".into(),
        DatasourceConfig {
            kind: DatasourceKind::Sqlite,
            mode: TruthMode::ManagedPush,
            path: Some(db_path_von),
            url: None,
        },
    );
    project.save(&config).unwrap();

    let cfg = config.to_str().unwrap();

    let out = iris_bin()
        .args(["--config", cfg, "doctor"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("project:"));
    assert!(stdout.contains("main"));

    let out = iris_bin()
        .args(["--config", cfg, "inspect", "--source", "main"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);

    let plan_path = dir.join("migrations").join("main-plan.von");
    let out = iris_bin()
        .args([
            "--config",
            cfg,
            "migrate",
            "plan",
            "--source",
            "main",
            "--out",
            plan_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    assert!(plan_path.exists());
    let plan_text = std::fs::read_to_string(&plan_path).unwrap();
    assert!(
        plan_text.contains("CreateTable")
            || plan_text.contains("create_table")
            || plan_text.contains("User")
    );

    let out = iris_bin()
        .args([
            "--config",
            cfg,
            "migrate",
            "apply",
            "--source",
            "main",
            "--plan",
            plan_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!out.status.success(), "apply without --yes must fail");

    let out = iris_bin()
        .args([
            "--config",
            cfg,
            "migrate",
            "apply",
            "--source",
            "main",
            "--plan",
            plan_path.to_str().unwrap(),
            "--yes",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);

    let out = iris_bin()
        .args(["--config", cfg, "migrate", "verify", "--source", "main"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    assert!(String::from_utf8_lossy(&out.stdout).contains("verify ok"));

    let mapping = dir.join("mappings").join("main.von");
    let out = iris_bin()
        .args([
            "--config",
            cfg,
            "adopt",
            "plan",
            "--source",
            "main",
            "--out",
            mapping.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let mapping_text = std::fs::read_to_string(&mapping).unwrap();
    assert!(mapping_text.contains("User"));
    assert!(!mapping_text.to_ascii_lowercase().contains("create table"));

    let out = iris_bin()
        .args(["--config", cfg, "inspect", "--source", "main"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    assert!(String::from_utf8_lossy(&out.stdout).contains("User"));

    let out = iris_bin()
        .args(["--config", cfg, "generate"])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    assert!(dir.join(".cache").join("iris").join("iris.lock").exists());
    assert!(
        dir.join(".cache")
            .join("iris")
            .join("generate")
            .join("mod.rs")
            .exists()
    );

    let _ = std::fs::remove_dir_all(dir);
}
