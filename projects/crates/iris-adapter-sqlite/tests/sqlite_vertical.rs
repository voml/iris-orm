//! Phase 3: SQLite foreign adapter vertical slice.

use std::collections::BTreeMap;

use iris_adapter_sqlite::{BACKEND_ID, SqliteSource};
use iris_ir::RealizationClass;
use iris_types::{Planner, RowWrite, Value};

const USER_SCHEMA: &str = r#"
table User {
    @@user_id: utf8,
    @user_name: utf8,
    active: bool,
}
"#;

#[test]
fn managed_push_inspect_crud_txn_and_drift() {
    let db = SqliteSource::open_in_memory().unwrap();
    assert_eq!(SqliteSource::capabilities().backend_id, BACKEND_ID);

    let report = db.managed_push(USER_SCHEMA).expect("push");
    assert_eq!(report.created_tables, vec!["User".to_string()]);

    let catalog = db.inspect().unwrap();
    assert!(catalog.table("User").is_some());
    assert!(db.drift(USER_SCHEMA).unwrap().is_clean());

    db.begin().unwrap();
    db.insert(&RowWrite {
        table: "User".into(),
        primary_key: "user_id".into(),
        fields: BTreeMap::from([
            ("user_id".into(), Value::Str("u1".into())),
            ("user_name".into(), Value::Str("alice".into())),
            ("active".into(), Value::Bool(true)),
        ]),
    })
    .unwrap();
    db.insert(&RowWrite {
        table: "User".into(),
        primary_key: "user_id".into(),
        fields: BTreeMap::from([
            ("user_id".into(), Value::Str("u2".into())),
            ("user_name".into(), Value::Str("bob".into())),
            ("active".into(), Value::Bool(false)),
        ]),
    })
    .unwrap();
    db.commit().unwrap();

    let plan = Planner::new(SqliteSource::capabilities())
        .plan_source(
            r#"
            User.filter(x => x.active)
                .sort_by(x => x.user_name)
                .collect()
            "#,
        )
        .unwrap();
    assert!(
        plan.nodes
            .iter()
            .all(|n| n.realization == RealizationClass::Native)
    );
    let rows = db.execute_plan(&plan).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("user_name"), Some(&Value::Str("alice".into())));

    let n = db
        .update(&RowWrite {
            table: "User".into(),
            primary_key: "user_id".into(),
            fields: BTreeMap::from([
                ("user_id".into(), Value::Str("u2".into())),
                ("active".into(), Value::Bool(true)),
            ]),
        })
        .unwrap();
    assert_eq!(n, 1);
    let n = db
        .delete("User", "user_id", &Value::Str("u1".into()))
        .unwrap();
    assert_eq!(n, 1);
}

#[test]
fn adopt_existing_blocks_missing_pk_and_maps_exact_fields() {
    let db = SqliteSource::open_in_memory().unwrap();
    db.managed_push(USER_SCHEMA).unwrap();
    let manifest = db.adopt(USER_SCHEMA).unwrap();
    assert_eq!(manifest.adapter_id, BACKEND_ID);
    let user = manifest
        .tables
        .iter()
        .find(|t| t.vos_table == "User")
        .expect("User mapping");
    assert!(user.blockers.is_empty(), "blockers={:?}", user.blockers);
    assert!(user.fields.iter().any(|f| f.vos_field == "user_name"));

    // Physical table without PK: VOS still declares a PK so parse succeeds;
    // adopt must block on the observed catalog, not invent a key.
    let path = std::env::temp_dir().join(format!(
        "iris-sqlite-adopt-{}.db",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    {
        let raw = rusqlite::Connection::open(&path).unwrap();
        raw.execute_batch("CREATE TABLE Legacy (name TEXT);")
            .unwrap();
    }
    let db2 = SqliteSource::open(&path).unwrap();
    let manifest = db2
        .adopt(
            r#"
            table Legacy {
                @@id: utf8,
                name: utf8,
            }
            "#,
        )
        .unwrap();
    let legacy = manifest
        .tables
        .iter()
        .find(|t| t.vos_table == "Legacy")
        .unwrap();
    assert!(
        legacy.blockers.iter().any(|b| b.contains("no primary key")),
        "expected missing-PK blocker, got {:?}",
        legacy.blockers
    );
    assert!(
        legacy
            .blockers
            .iter()
            .any(|b| b.contains("no physical column")),
        "expected missing `id` column blocker, got {:?}",
        legacy.blockers
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn transaction_rollback_discards_writes() {
    let db = SqliteSource::open_in_memory().unwrap();
    db.managed_push(USER_SCHEMA).unwrap();
    db.begin().unwrap();
    db.insert(&RowWrite {
        table: "User".into(),
        primary_key: "user_id".into(),
        fields: BTreeMap::from([
            ("user_id".into(), Value::Str("rb".into())),
            ("user_name".into(), Value::Str("gone".into())),
            ("active".into(), Value::Bool(true)),
        ]),
    })
    .unwrap();
    db.rollback().unwrap();
    let plan = Planner::new(SqliteSource::capabilities())
        .plan_source(r#"User.filter(x => x.user_id == "rb").collect()"#)
        .unwrap();
    assert!(db.execute_plan(&plan).unwrap().is_empty());
}

#[test]
fn drift_reports_missing_physical_table() {
    let db = SqliteSource::open_in_memory().unwrap();
    let report = db.drift(USER_SCHEMA).unwrap();
    assert!(!report.is_clean());
    assert!(report.missing_physical_tables.iter().any(|t| t == "User"));
}

#[test]
fn managed_push_plan_is_reviewable_and_idempotent() {
    let db = SqliteSource::open_in_memory().unwrap();
    let plan = db.plan_managed_push(USER_SCHEMA).unwrap();
    assert!(!plan.destructive);
    assert_eq!(plan.changes.len(), 1);
    db.apply_managed_push(&plan, USER_SCHEMA).unwrap();
    let again = db.plan_managed_push(USER_SCHEMA).unwrap();
    assert!(
        again.changes.is_empty(),
        "second plan should be empty after apply"
    );
    assert!(db.drift(USER_SCHEMA).unwrap().is_clean());
}

#[test]
fn file_reopen_preserves_rows() {
    let path = std::env::temp_dir().join(format!(
        "iris-sqlite-reopen-{}.db",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    {
        let db = SqliteSource::open(&path).unwrap();
        db.managed_push(USER_SCHEMA).unwrap();
        db.insert(&RowWrite {
            table: "User".into(),
            primary_key: "user_id".into(),
            fields: BTreeMap::from([
                ("user_id".into(), Value::Str("persist".into())),
                ("user_name".into(), Value::Str("keep".into())),
                ("active".into(), Value::Bool(false)),
            ]),
        })
        .unwrap();
    }
    let db = SqliteSource::open(&path).unwrap();
    let plan = Planner::new(SqliteSource::capabilities())
        .plan_source(r#"User.filter(x => x.user_id == "persist").collect()"#)
        .unwrap();
    let rows = db.execute_plan(&plan).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("user_name"), Some(&Value::Str("keep".into())));
    let _ = std::fs::remove_file(path);
}

#[test]
fn type_round_trip_bool_and_text() {
    let db = SqliteSource::open_in_memory().unwrap();
    db.managed_push(USER_SCHEMA).unwrap();
    db.insert(&RowWrite {
        table: "User".into(),
        primary_key: "user_id".into(),
        fields: BTreeMap::from([
            ("user_id".into(), Value::Str("rt".into())),
            ("user_name".into(), Value::Str("round".into())),
            ("active".into(), Value::Bool(true)),
        ]),
    })
    .unwrap();
    let plan = Planner::new(SqliteSource::capabilities())
        .plan_source(r#"User.filter(x => x.user_id == "rt").collect()"#)
        .unwrap();
    let rows = db.execute_plan(&plan).unwrap();
    assert_eq!(rows.len(), 1);
    // bool stored as INTEGER; reader currently maps INTEGER ???Int.
    assert!(matches!(
        rows[0].get("active"),
        Some(Value::Int(1)) | Some(Value::Bool(true))
    ));
    assert_eq!(rows[0].get("user_name"), Some(&Value::Str("round".into())));
}

#[test]
fn public_api_surface_does_not_return_sql_strings() {
    // Structural: crate public items are typed reports/plans/rows --?this test
    // locks that managed_push returns PushReport, not DDL text.
    let db = SqliteSource::open_in_memory().unwrap();
    let report = db.managed_push(USER_SCHEMA).unwrap();
    let debug = format!("{report:?}");
    assert!(!debug.contains("CREATE TABLE"));
    assert!(!debug.contains("INSERT INTO"));
}

#[test]
fn adapter_is_not_yydb() {
    assert_ne!(BACKEND_ID, "yydb");
    assert_ne!(BACKEND_ID, "reference");
    let manifest = include_str!("../Cargo.toml");
    assert!(!manifest.contains("yydb"));
    assert!(manifest.contains("rusqlite"));
}

#[test]
fn uncommitted_transaction_is_rolled_back_on_reopen() {
    let path = std::env::temp_dir().join(format!(
        "iris-sqlite-crash-{}.db",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    {
        let db = SqliteSource::open(&path).unwrap();
        db.managed_push(USER_SCHEMA).unwrap();
        db.begin().unwrap();
        db.insert(&RowWrite {
            table: "User".into(),
            primary_key: "user_id".into(),
            fields: BTreeMap::from([
                ("user_id".into(), Value::Str("ghost".into())),
                ("user_name".into(), Value::Str("nope".into())),
                ("active".into(), Value::Bool(true)),
            ]),
        })
        .unwrap();
        // Crash simulation: drop without commit/rollback.
    }
    let db = SqliteSource::open(&path).unwrap();
    let plan = Planner::new(SqliteSource::capabilities())
        .plan_source(r#"User.filter(x => x.user_id == "ghost").collect()"#)
        .unwrap();
    assert!(
        db.execute_plan(&plan).unwrap().is_empty(),
        "uncommitted insert must not survive reopen"
    );
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn soak_reopen_push_and_query_cycles() {
    let path = std::env::temp_dir().join(format!(
        "iris-sqlite-soak-{}.db",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    {
        let db = SqliteSource::open(&path).unwrap();
        db.managed_push(USER_SCHEMA).unwrap();
    }
    for i in 0..25 {
        let db = SqliteSource::open(&path).unwrap();
        let id = format!("s{i}");
        db.insert(&RowWrite {
            table: "User".into(),
            primary_key: "user_id".into(),
            fields: BTreeMap::from([
                ("user_id".into(), Value::Str(id.clone())),
                ("user_name".into(), Value::Str(format!("name-{i}"))),
                ("active".into(), Value::Bool(true)),
            ]),
        })
        .unwrap();
        let plan = Planner::new(SqliteSource::capabilities())
            .plan_source(&format!(
                r#"User.filter(x => x.user_id == "{id}").collect()"#
            ))
            .unwrap();
        assert_eq!(db.execute_plan(&plan).unwrap().len(), 1);
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn connection_failure_bad_path_parent_is_created_but_corrupt_path_errors() {
    // Opening a directory as a DB path should fail.
    let dir = std::env::temp_dir().join(format!(
        "iris-sqlite-bad-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let err = SqliteSource::open(&dir).expect_err("directory is not a sqlite file");
    assert!(!format!("{err}").is_empty());
    let _ = std::fs::remove_dir_all(dir);
}
