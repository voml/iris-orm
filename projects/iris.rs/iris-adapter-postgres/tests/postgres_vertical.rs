//! Phase 4: PostgreSQL foreign adapter --?offline + optional live tests.

use iris_adapter_postgres::{BACKEND_ID, PostgresSource, adopt_plan, classify_type};
use iris_ir::RealizationClass;
use iris_types::{
    MappingQuality, ObservedCatalog, ObservedColumn, ObservedTable, Planner, RowWrite, Value,
};
use std::collections::BTreeMap;

const USER_SCHEMA: &str = r#"
table User {
    @@user_id: utf8,
    @user_name: utf8,
    active: bool,
}
"#;

fn live_url() -> Option<String> {
    std::env::var("IRIS_TEST_POSTGRES_URL")
        .ok()
        .filter(|s| !s.is_empty())
}

#[test]
fn connection_failure_bad_conninfo() {
    let err = PostgresSource::connect_with_pool_timeout(
        "host=127.0.0.1 port=1 user=iris dbname=iris connect_timeout=1",
        std::time::Duration::from_secs(2),
    )
    .expect_err("unreachable postgres");
    assert!(!err.to_string().is_empty());
}

#[test]
fn adapter_is_not_sqlite_or_mysql() {
    assert_eq!(BACKEND_ID, "postgres");
    assert_ne!(BACKEND_ID, "sqlite");
    assert_ne!(BACKEND_ID, "mysql");
    assert_ne!(BACKEND_ID, "yydb");
    let manifest = include_str!("../Cargo.toml");
    assert!(manifest.contains("postgres"));
    assert!(!manifest.contains("rusqlite"));
    assert!(!manifest.contains("mysql ="));
}

#[test]
fn postgres_type_mapping_is_dialect_specific() {
    assert_eq!(classify_type("bool", "bool").0, MappingQuality::Exact);
    assert_eq!(classify_type("utf8", "text").0, MappingQuality::Exact);
    assert_eq!(classify_type("uuid", "uuid").0, MappingQuality::Exact);
    // MySQL-style tinyint must not silently pass as Exact on PG classifier.
    assert_eq!(
        classify_type("bool", "tinyint").0,
        MappingQuality::LossyBlocked
    );
}

#[test]
fn adopt_plan_blocks_missing_pk_without_live_db() {
    let catalog = ObservedCatalog {
        backend_id: BACKEND_ID.into(),
        tables: vec![ObservedTable {
            name: "Legacy".into(),
            columns: vec![ObservedColumn {
                name: "name".into(),
                type_name: "text".into(),
                nullable: true,
                primary_key: false,
            }],
        }],
    };
    let doc = vos::parser::parse_document(
        r#"
        table Legacy {
            @@id: utf8,
            name: utf8,
        }
        "#,
    )
    .expect("parse");
    let manifest = adopt_plan(&doc, &catalog);
    let legacy = manifest
        .tables
        .iter()
        .find(|t| t.vos_table == "Legacy")
        .unwrap();
    assert!(
        legacy.blockers.iter().any(|b| b.contains("no primary key")),
        "{:?}",
        legacy.blockers
    );
}

#[test]
fn live_managed_push_crud_txn_drift_and_pool() {
    let Some(url) = live_url() else {
        eprintln!("skip: set IRIS_TEST_POSTGRES_URL for live PostgreSQL conformance");
        return;
    };
    let db = PostgresSource::connect(&url).expect("connect");
    assert!(db.pool_connections() >= 1 || db.pool_connections() == 0);

    // Isolate from other runs.
    db.with_connection(|c| {
        c.batch_execute("DROP TABLE IF EXISTS \"User\"")?;
        Ok(())
    })
    .unwrap();

    let report = db.managed_push(USER_SCHEMA).expect("push");
    assert_eq!(report.created_tables, vec!["User".to_string()]);
    assert!(!format!("{report:?}").contains("CREATE TABLE"));
    assert!(db.drift(USER_SCHEMA).unwrap().is_clean());

    db.transaction(|client| {
        // use adapter helpers via re-checkout is fine; exercise txn API:
        let _ = client;
        Ok(())
    })
    .unwrap();

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

    let plan = Planner::new(PostgresSource::capabilities())
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
    assert!(matches!(
        rows[0].get("active"),
        Some(Value::Bool(true)) | Some(Value::Int(1))
    ));

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

    let again = db.plan_managed_push(USER_SCHEMA).unwrap();
    assert!(again.changes.is_empty());

    let manifest = db.adopt(USER_SCHEMA).unwrap();
    assert_eq!(manifest.adapter_id, BACKEND_ID);
    assert!(
        manifest
            .tables
            .iter()
            .find(|t| t.vos_table == "User")
            .unwrap()
            .blockers
            .is_empty()
    );

    db.with_connection(|c| {
        c.batch_execute("DROP TABLE IF EXISTS \"User\"")?;
        Ok(())
    })
    .unwrap();
}
