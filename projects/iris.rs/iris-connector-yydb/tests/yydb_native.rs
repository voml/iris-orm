//! Phase 2: native YYDB connector readiness + schema handshake.

use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use iris_connector_yydb::{BACKEND_ID, READINESS_CODE, YydbSource};

const USER_SCHEMA: &str = r#"
table User {
    @@user_id: uuid,
    @user_name: utf8,
    active: bool,
}
"#;

fn temp_db_path(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("iris-yydb-{label}-{nanos}"));
    fs::create_dir_all(&dir).expect("temp dir");
    dir.join("db.yydb")
}

#[test]
fn readiness_probe_is_not_ready_for_vos_executor() {
    let report = YydbSource::readiness();
    assert_eq!(report.backend_id, BACKEND_ID);
    assert_eq!(report.code, READINESS_CODE);
    assert!(report.schema_handshake_ready);
    assert!(!report.vos_executor_ready);
    assert!(!report.is_ready());
}

#[test]
fn ensure_schema_and_handshake_work_on_public_facade() {
    let db = YydbSource::open_in_memory().expect("open");
    assert_eq!(YydbSource::capabilities().backend_id, BACKEND_ID);

    db.ensure_schema(1, USER_SCHEMA).expect("schema");
    let hs = db.schema_handshake().expect("handshake");
    assert_eq!(hs.backend_id, BACKEND_ID);
    assert_eq!(hs.schema_version, Some(1));
    assert!(hs.has_document);
    assert!(hs.ddl_revision >= 1);
}

#[test]
fn query_refuses_until_yydb_executor_ships() {
    let db = YydbSource::open_in_memory().unwrap();
    db.ensure_schema(1, USER_SCHEMA).unwrap();
    let err = db
        .query(r#"User.filter(x => true).collect()"#)
        .expect_err("must not query yet");
    assert!(err.to_string().contains(READINESS_CODE));
}

#[test]
fn reopen_preserves_schema_document() {
    let path = temp_db_path("reopen");
    {
        let db = YydbSource::open(&path).unwrap();
        db.ensure_schema(1, USER_SCHEMA).unwrap();
    }
    assert!(fs::metadata(&path).is_ok());
    let db = YydbSource::open(&path).unwrap();
    let hs = db.schema_handshake().unwrap();
    assert_eq!(hs.schema_version, Some(1));
    assert!(hs.has_document);

    let db = db.reopen().unwrap();
    let hs = db.schema_handshake().unwrap();
    assert_eq!(hs.schema_version, Some(1));
    let _ = fs::remove_file(&path);
}

#[test]
fn connector_does_not_depend_on_foreign_adapters() {
    let manifest = include_str!("../Cargo.toml");
    for banned in [
        "iris-adapter-mysql",
        "iris-adapter-postgres",
        "iris-adapter-sqlite",
        "iris-adapter-redis",
        "rusqlite",
        "sqlx",
    ] {
        assert!(
            !manifest.contains(banned),
            "native YYDB connector must not depend on `{banned}`"
        );
    }
    assert!(manifest.contains("yydb"));
}
