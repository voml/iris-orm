//! Phase 2: native YYDB connector integration.

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
fn create_open_schema_handshake_and_crud() {
    let db = YydbSource::open_in_memory().expect("open");
    assert_eq!(YydbSource::capabilities().backend_id, BACKEND_ID);

    db.ensure_schema(1, USER_SCHEMA).expect("schema");
    let hs = db.schema_handshake().expect("handshake");
    assert_eq!(hs.backend_id, BACKEND_ID);
    assert_eq!(hs.schema_version, Some(1));
    assert!(hs.has_document);
    assert!(hs.ddl_revision >= 1);

    db.execute_vos(
        r#"
        User { user_id: uuid(), user_name: "alice", active: true }.insert()
        "#,
    )
    .expect("insert alice");
    db.execute_vos(
        r#"
        User { user_id: uuid(), user_name: "bob", active: false }.insert()
        "#,
    )
    .expect("insert bob");

    let rows = db
        .execute_vos(
            r#"
            User.filter(x => x.active)
                .sort_by(x => x.user_name)
                .collect()
            "#,
        )
        .expect("query");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].get("user_name").map(|v| format!("{v:?}")),
        Some(r#"Str("alice")"#.into())
    );
}

#[test]
fn transaction_commit_and_rollback() {
    let db = YydbSource::open_in_memory().unwrap();
    db.ensure_schema(1, USER_SCHEMA).unwrap();

    db.begin().unwrap();
    db.execute_vos(r#"User { user_id: uuid(), user_name: "txn", active: true }.insert()"#)
        .unwrap();
    assert!(db.in_transaction());
    db.commit().unwrap();
    assert!(!db.in_transaction());
    let rows = db
        .execute_vos(r#"User.filter(x => x.user_name == "txn").collect()"#)
        .unwrap();
    assert_eq!(rows.len(), 1);

    db.begin().unwrap();
    db.execute_vos(r#"User { user_id: uuid(), user_name: "gone", active: true }.insert()"#)
        .unwrap();
    db.rollback().unwrap();
    let rows = db
        .execute_vos(r#"User.filter(x => x.user_name == "gone").collect()"#)
        .unwrap();
    assert!(rows.is_empty());
}

#[test]
fn reopen_preserves_data() {
    let path = temp_db_path("reopen");
    {
        let db = YydbSource::open(&path).unwrap();
        db.ensure_schema(1, USER_SCHEMA).unwrap();
        db.execute_vos(r#"User { user_id: uuid(), user_name: "persist", active: true }.insert()"#)
            .unwrap();
    }
    assert!(fs::metadata(&path).is_ok());
    let db = YydbSource::open(&path).unwrap();
    let hs = db.schema_handshake().unwrap();
    assert_eq!(hs.schema_version, Some(1));
    let rows = db
        .execute_vos(r#"User.filter(x => x.user_name == "persist").collect()"#)
        .unwrap();
    assert_eq!(rows.len(), 1);

    let db = db.reopen().unwrap();
    let rows = db
        .execute_vos(r#"User.filter(x => x.user_name == "persist").collect()"#)
        .unwrap();
    assert_eq!(rows.len(), 1);
    let _ = fs::remove_file(&path);
}

#[test]
fn prepared_invalidates_after_ddl() {
    let db = YydbSource::open_in_memory().unwrap();
    db.ensure_schema(1, "table User { @@user_id: uuid, @user_name: utf8 }")
        .unwrap();
    let prepared = db.prepare(r#"User.filter(x => true).collect()"#).unwrap();
    let rev = prepared.ddl_revision();
    assert_eq!(rev, db.schema_handshake().unwrap().ddl_revision);

    {
        let ddl = db.connection().begin_ddl_session().unwrap();
        ddl.rename_field("User", "user_name", "display_name")
            .unwrap();
        ddl.commit().unwrap();
    }
    let err = prepared.execute(&db).expect_err("prepared must go stale");
    assert!(err.to_string().contains("VOS-PREPARED-STALE"), "err={err}");
}

#[test]
fn query_session_stale_after_ddl() {
    let db = YydbSource::open_in_memory().unwrap();
    db.ensure_schema(1, "table User { @@user_id: uuid, @user_name: utf8 }")
        .unwrap();
    let session = db.connection().begin_query_session().unwrap();
    {
        let ddl = db.connection().begin_ddl_session().unwrap();
        ddl.rename_field("User", "user_name", "display_name")
            .unwrap();
        ddl.commit().unwrap();
    }
    let err = session
        .query("User.filter(x => true).collect()")
        .expect_err("session stale");
    assert!(err.to_string().contains("VOS-SESSION-STALE"));
}

#[test]
fn second_writer_sees_busy_boundary() {
    let path = temp_db_path("busy");
    let a = YydbSource::open(&path).unwrap();
    a.ensure_schema(1, USER_SCHEMA).unwrap();
    a.begin().unwrap();

    match YydbSource::open(&path) {
        Ok(b) => {
            let err = b.begin().expect_err("second writer begin must be busy");
            let msg = err.to_string();
            assert!(
                msg.to_ascii_lowercase().contains("busy"),
                "expected busy boundary, got {msg}"
            );
        }
        Err(err) => {
            let msg = err.to_string();
            assert!(
                msg.to_ascii_lowercase().contains("busy"),
                "expected busy on second open, got {msg}"
            );
        }
    }
    a.rollback().unwrap();
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
