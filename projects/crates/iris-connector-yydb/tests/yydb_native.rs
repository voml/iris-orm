//! Phase 2: native YYDB connector readiness + schema handshake.

use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use iris_connector_yydb::{BACKEND_ID, YydbSource};

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
fn readiness_probe_reports_vos_query_executor() {
    let report = YydbSource::readiness();
    assert_eq!(report.backend_id, BACKEND_ID);
    assert!(report.schema_handshake_ready);
    assert!(report.vos_executor_ready);
    assert!(report.is_ready());
    assert_eq!(report.code, "IRIS-YYDB-VOS-EXECUTOR-READY");
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
fn query_roundtrips_active_users() {
    use std::collections::BTreeMap;

    let db = YydbSource::open_in_memory().unwrap();
    db.ensure_schema(1, USER_SCHEMA).unwrap();
    db.connection()
        .upsert_row(
            "User",
            "ada",
            BTreeMap::from([
                ("user_name".into(), yydb::Value::Text("ada".into())),
                ("active".into(), yydb::Value::Bool(true)),
            ]),
        )
        .unwrap();
    db.connection()
        .upsert_row(
            "User",
            "grace",
            BTreeMap::from([
                ("user_name".into(), yydb::Value::Text("grace".into())),
                ("active".into(), yydb::Value::Bool(false)),
            ]),
        )
        .unwrap();

    let rows = db
        .query(r#"User.filter(x => x.active).collect()"#)
        .expect("query");
    assert_eq!(rows.len(), 1);
    assert!(matches!(
        rows[0].get("user_name"),
        Some(iris_types::Value::Str(name)) if name == "ada"
    ));
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
