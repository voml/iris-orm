//! Phase 2: native YYDB connector readiness + schema handshake.

use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use iris_connector_yydb::{BACKEND_ID, PREPARED_STALE_CODE, SESSION_STALE_CODE, YydbSource};

const USER_SCHEMA: &str = r#"
table User {
    @@user_id: uuid,
    @user_name: utf8,
    active: bool,
}
"#;

const BLOG_SCHEMA: &str = r#"
table User {
    @@user_id: uuid,
    user_name: utf8,
    active: bool,
}

table Post {
    @@post_id: uuid,
    author: &User,
    title: utf8,
    published: bool,
}
"#;

fn temp_db_path(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
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

    db.ensure_schema(USER_SCHEMA).expect("schema");
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
    db.ensure_schema(USER_SCHEMA).unwrap();
    db.connection()
        .upsert_row(
            "User",
            "ada",
            BTreeMap::from([("user_name".into(), yydb::Value::Text("ada".into())), ("active".into(), yydb::Value::Bool(true))]),
        )
        .unwrap();
    db.connection()
        .upsert_row(
            "User",
            "grace",
            BTreeMap::from([("user_name".into(), yydb::Value::Text("grace".into())), ("active".into(), yydb::Value::Bool(false))]),
        )
        .unwrap();

    let rows = db.query(r#"User.filter(x => x.active).collect()"#).expect("query");
    assert_eq!(rows.len(), 1);
    assert!(matches!(
        rows[0].get("user_name"),
        Some(iris_types::Value::Str(name)) if name == "ada"
    ));
}

#[test]
fn execute_runs_seed_blog_insert_program() {
    let db = YydbSource::open_in_memory().unwrap();
    db.ensure_schema(BLOG_SCHEMA).unwrap();
    db.execute(
        r#"
        User {
            user_id: "550e8400-e29b-41d4-a716-446655440000",
            user_name: "ada",
            active: true,
        }.insert()
        User {
            user_id: "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
            user_name: "linus",
            active: true,
        }.insert()
        Post {
            post_id: "11111111-1111-4111-8111-111111111101",
            author: "550e8400-e29b-41d4-a716-446655440000",
            title: "Ada post",
            published: true,
        }.insert()
        "#,
    )
    .expect("execute");

    let users = db.query(r#"User.filter(x => true).collect()"#).expect("users");
    let posts = db.query(r#"Post.filter(x => x.published).collect()"#).expect("posts");
    assert_eq!(users.len(), 2);
    assert_eq!(posts.len(), 1);
}

#[test]
fn transaction_commit_makes_execute_visible() {
    let db = YydbSource::open_in_memory().unwrap();
    db.ensure_schema(BLOG_SCHEMA).unwrap();
    db.begin().unwrap();
    assert!(db.in_transaction());
    db.execute(
        r#"User {
            user_id: "550e8400-e29b-41d4-a716-446655440000",
            user_name: "ada",
            active: true,
        }.insert()"#,
    )
    .unwrap();
    db.commit().unwrap();
    assert!(!db.in_transaction());

    let rows = db.query(r#"User.filter(x => true).collect()"#).expect("users");
    assert_eq!(rows.len(), 1);
}

#[test]
fn prepared_query_executes_when_ddl_revision_matches() {
    let db = YydbSource::open_in_memory().unwrap();
    db.ensure_schema(USER_SCHEMA).unwrap();
    db.connection()
        .upsert_row(
            "User",
            "ada",
            [("user_name".into(), yydb::Value::Text("ada".into())), ("active".into(), yydb::Value::Bool(true))].into_iter().collect(),
        )
        .unwrap();

    let prepared = db.prepare(r#"User.filter(x => x.active).collect()"#).expect("prepare");
    assert_eq!(prepared.ddl_revision(), 1);
    let rows = prepared.execute(&db).expect("execute prepared");
    assert_eq!(rows.len(), 1);
}

#[test]
fn prepared_execute_rejects_stale_ddl_revision() {
    let db = YydbSource::open_in_memory().unwrap();
    db.ensure_schema(USER_SCHEMA).unwrap();
    let prepared = db.prepare(r#"User.filter(x => true).collect()"#).expect("prepare");
    db.connection().migrate_schema(1, 2, USER_SCHEMA, &Default::default()).expect("migrate");

    let err = prepared.execute(&db).expect_err("stale prepared");
    assert!(err.to_string().contains(PREPARED_STALE_CODE));
}

#[test]
fn session_ddl_revision_rejects_stale_sessions() {
    let db = YydbSource::open_in_memory().unwrap();
    db.ensure_schema(USER_SCHEMA).unwrap();
    let revision = db.schema_handshake().unwrap().ddl_revision;
    db.check_session_ddl_revision(revision).expect("fresh session");
    db.connection().migrate_schema(1, 2, USER_SCHEMA, &Default::default()).expect("migrate");
    let err = db.check_session_ddl_revision(revision).expect_err("stale session");
    assert!(err.to_string().contains(SESSION_STALE_CODE));
}

#[test]
fn execute_inserts_via_static_insert_program() {
    let db = YydbSource::open_in_memory().unwrap();
    db.ensure_schema(USER_SCHEMA).unwrap();
    db.execute(
        r#"User::insert({
            user_id: "550e8400-e29b-41d4-a716-446655440000",
            user_name: "ada",
            active: true,
        })"#,
    )
    .expect("execute");

    let rows = db.query(r#"User.filter(x => true).collect()"#).expect("query");
    assert_eq!(rows.len(), 1);
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
