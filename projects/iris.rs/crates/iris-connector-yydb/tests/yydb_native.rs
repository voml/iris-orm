//! Phase 2: native YYDB connector — VOS executor live on the Connection facade.

use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use iris_connector_yydb::{BACKEND_ID, YydbSource};

const USER_SCHEMA: &str = r#"
table User {
    @@user_id: utf8,
    @user_name: utf8,
    active: bool,
}
"#;

fn temp_db_path(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("iris-yydb-{label}-{nanos}"));
    fs::create_dir_all(&dir).expect("temp dir");
    dir.join("db.yydb")
}

// ---------------------------------------------------------------------------
// Readiness probe — VOS executor is LIVE now
// ---------------------------------------------------------------------------

#[test]
fn readiness_probe_reports_vos_executor_ready() {
    let report = YydbSource::readiness();
    assert_eq!(report.backend_id, BACKEND_ID);
    assert!(report.schema_handshake_ready);
    assert!(report.vos_executor_ready);
    assert!(report.is_ready());
    assert!(report.message.contains("Connection"), "{}", report.message);
}

#[test]
fn ensure_schema_and_handshake_work_on_public_facade() {
    let db = YydbSource::open_in_memory().expect("open");
    assert_eq!(YydbSource::capabilities().backend_id, BACKEND_ID);

    db.ensure_schema(USER_SCHEMA).expect("schema");
    let hs = db.schema_handshake().expect("handshake");
    assert_eq!(hs.backend_id, BACKEND_ID);
    assert_eq!(hs.schema_version, Some(1));
    assert!(hs.has_document);
    assert!(hs.ddl_revision >= 1);
}

// ---------------------------------------------------------------------------
// Live VOS executor: query / execute / prepare / transaction
// ---------------------------------------------------------------------------

#[test]
fn vos_execute_inserts_a_row_and_query_finds_it() {
    let db = YydbSource::open_in_memory().unwrap();
    db.ensure_schema(USER_SCHEMA).unwrap();

    // DML via execute
    db.execute(
        r#"User {
            user_id: "550e8400-e29b-41d4-a716-446655440000",
            user_name: "ada",
            active: true,
        }.insert()"#,
    )
    .expect("insert");

    // Query via query
    let rows = db.query(r#"User.filter(x => true).collect()"#).expect("query");
    assert_eq!(rows.len(), 1);
    let first = &rows[0];
    assert_eq!(
        first.get("user_name").and_then(|v| match v {
            iris_types::Value::Str(s) => Some(s.as_str()),
            _ => None,
        }),
        Some("ada")
    );
    assert_eq!(first.get("active"), Some(&iris_types::Value::Bool(true)));
}

#[test]
fn vos_prepared_statement_executes_and_stale_checked() {
    let db = YydbSource::open_in_memory().unwrap();
    db.ensure_schema(USER_SCHEMA).unwrap();

    // Insert two users
    db.execute(r#"User { user_id: "11111111-1111-1111-1111-111111111111", user_name: "ada", active: true }.insert()"#).unwrap();
    db.execute(r#"User { user_id: "22222222-2222-2222-2222-222222222222", user_name: "bob", active: false }.insert()"#).unwrap();

    // Prepare a read plan
    let plan = db.prepare(r#"User.filter(x => x.active == true).collect()"#).expect("prepare");
    assert_eq!(plan.ddl_revision(), 1);

    let rows = plan.execute(&db).expect("execute");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].get("user_name").and_then(|v| match v {
            iris_types::Value::Str(s) => Some(s.as_str()),
            _ => None,
        }),
        Some("ada")
    );
}

#[test]
fn vos_prepared_stale_when_schema_drifts() {
    let db = YydbSource::open_in_memory().unwrap();
    db.ensure_schema(USER_SCHEMA).unwrap();

    let plan = db.prepare(r#"User.filter(x => true).collect()"#).unwrap();
    assert_eq!(plan.ddl_revision(), 1);

    // Directly mutate the schema stored in the YydbSource by re-opening and
    // using YYDB's migrate_schema — same semantics as a DDL revision bump.
    // We use YydbSource's escape hatch for this connector-local test since
    // migrate_schema hasn't been wrapped yet.
    let conn = db.connection();
    conn.migrate_schema(
        r#"
table User {
    @@user_id: utf8,
    @user_name: utf8,
    active: bool,
    extra: utf8,
}
"#,
        &Default::default(),
    )
    .unwrap();

    // ddl_revision now at least 2
    let hs = db.schema_handshake().unwrap();
    assert!(hs.ddl_revision > plan.ddl_revision());

    let err = plan.execute(&db).expect_err("stale plan must refuse");
    let msg = err.to_string();
    assert!(msg.contains("VOS-PREPARED-STALE"), "{msg}");
}

#[test]
fn vos_transaction_begin_commit_rollback() {
    let db = YydbSource::open_in_memory().unwrap();
    db.ensure_schema(USER_SCHEMA).unwrap();

    // begin → insert → commit
    db.begin().unwrap();
    db.execute(r#"User { user_id: "33333333-3333-3333-3333-333333333333", user_name: "charlie", active: true }.insert()"#).unwrap();
    db.commit().unwrap();

    let rows = db.query(r#"User.filter(x => x.user_name == "charlie").collect()"#).unwrap();
    assert_eq!(rows.len(), 1);

    // begin → insert → rollback → row disappears
    db.begin().unwrap();
    db.execute(r#"User { user_id: "44444444-4444-4444-4444-444444444444", user_name: "dave", active: true }.insert()"#).unwrap();
    db.rollback().unwrap();

    let rows = db.query(r#"User.filter(x => x.user_name == "dave").collect()"#).unwrap();
    assert_eq!(rows.len(), 0);
}

#[test]
fn reopen_preserves_schema_document() {
    let path = temp_db_path("reopen");
    {
        let db = YydbSource::open(&path).unwrap();
        db.ensure_schema(USER_SCHEMA).unwrap();
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
    for banned in ["iris-adapter-mysql", "iris-adapter-postgres", "iris-adapter-sqlite", "iris-adapter-redis", "rusqlite", "sqlx"] {
        assert!(!manifest.contains(banned), "native YYDB connector must not depend on `{banned}`");
    }
    assert!(manifest.contains("yydb"));
}
