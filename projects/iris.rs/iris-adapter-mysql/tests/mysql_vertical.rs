//! Phase 4: MySQL foreign adapter --?offline + optional live tests.

use iris_adapter_mysql::{BACKEND_ID, MysqlSource, adopt_plan, classify_type};
use iris_ir::RealizationClass;
use iris_types::{
    MappingQuality, ObservedCatalog, ObservedColumn, ObservedTable, Planner, RowWrite, Value,
};
use std::collections::BTreeMap;

const USER_SCHEMA: &str = r#"
table User {
    @@user_id: uuid,
    @user_name: utf8,
    active: bool,
}
"#;

fn live_url() -> Option<String> {
    std::env::var("IRIS_TEST_MYSQL_URL")
        .ok()
        .filter(|s| !s.is_empty())
}

#[test]
fn adapter_is_not_sqlite_or_postgres() {
    assert_eq!(BACKEND_ID, "mysql");
    assert_ne!(BACKEND_ID, "sqlite");
    assert_ne!(BACKEND_ID, "postgres");
    assert_ne!(BACKEND_ID, "yydb");
    let manifest = include_str!("../Cargo.toml");
    assert!(manifest.contains("mysql"));
    assert!(!manifest.contains("rusqlite"));
    assert!(!manifest.contains("postgres"));
}

#[test]
fn mysql_type_mapping_is_dialect_specific() {
    assert_eq!(
        classify_type("bool", "tinyint").0,
        MappingQuality::Compatible
    );
    assert_eq!(classify_type("utf8", "text").0, MappingQuality::Exact);
    assert_eq!(classify_type("uuid", "char").0, MappingQuality::Compatible);
    assert_eq!(classify_type("uuid", "binary").0, MappingQuality::Exact);
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
fn plan_push_emits_add_field_for_missing_columns() {
    use iris_adapter_mysql::plan_push;
    use iris_types::LogicalChange;

    let catalog = ObservedCatalog {
        backend_id: BACKEND_ID.into(),
        tables: vec![ObservedTable {
            name: "Goods".into(),
            columns: vec![
                ObservedColumn {
                    name: "sku_id".into(),
                    type_name: "varchar".into(),
                    nullable: false,
                    primary_key: true,
                },
                ObservedColumn {
                    name: "name".into(),
                    type_name: "text".into(),
                    nullable: false,
                    primary_key: false,
                },
            ],
        }],
    };
    let doc = vos::parser::parse_document(
        r#"
        table Goods {
            @@sku_id: utf8,
            name: utf8,
            cover_url: utf8?,
            status: utf8,
        }
        "#,
    )
    .expect("parse");
    let plan = plan_push(&doc, &catalog).expect("plan");
    let adds: Vec<_> = plan
        .changes
        .iter()
        .filter_map(|c| match c {
            LogicalChange::AddField {
                vos_table,
                vos_field,
            } => Some((vos_table.as_str(), vos_field.as_str())),
            _ => None,
        })
        .collect();
    assert!(adds.contains(&("Goods", "cover_url")), "{adds:?}");
    assert!(adds.contains(&("Goods", "status")), "{adds:?}");
    assert!(!plan.changes.iter().any(|c| matches!(c, LogicalChange::CreateTable { .. })));
}

#[test]
fn live_managed_push_crud_txn_drift_and_pool() {
    let Some(url) = live_url() else {
        eprintln!("skip: set IRIS_TEST_MYSQL_URL for live MySQL conformance");
        return;
    };
    let db = MysqlSource::connect(&url).expect("connect");
    db.ping().expect("pool ping");
    let db = db.with_vos_schema(USER_SCHEMA).expect("uuid schema map");

    db.transaction(|conn| {
        use mysql::prelude::*;
        conn.query_drop("DROP TABLE IF EXISTS `User`")?;
        Ok(())
    })
    .unwrap();

    let report = db.managed_push(USER_SCHEMA).expect("push");
    assert_eq!(report.created_tables, vec!["User".to_string()]);
    assert!(!format!("{report:?}").contains("CREATE TABLE"));
    assert!(db.drift(USER_SCHEMA).unwrap().is_clean());

    db.insert(&RowWrite {
        table: "User".into(),
        primary_key: "user_id".into(),
        fields: BTreeMap::from([
            (
                "user_id".into(),
                Value::Str("550e8400-e29b-41d4-a716-446655440000".into()),
            ),
            ("user_name".into(), Value::Str("alice".into())),
            ("active".into(), Value::Bool(true)),
        ]),
    })
    .unwrap();
    db.insert(&RowWrite {
        table: "User".into(),
        primary_key: "user_id".into(),
        fields: BTreeMap::from([
            (
                "user_id".into(),
                Value::Str("6ba7b810-9dad-11d1-80b4-00c04fd430c8".into()),
            ),
            ("user_name".into(), Value::Str("bob".into())),
            ("active".into(), Value::Bool(false)),
        ]),
    })
    .unwrap();

    let plan = Planner::new(MysqlSource::capabilities())
        .plan_source(
            r#"
            User.filter(x => x.user_id == "550e8400-e29b-41d4-a716-446655440000")
                .collect()
            "#,
        )
        .unwrap();
    let plan_where = Planner::new(MysqlSource::capabilities())
        .plan_source(
            r#"
            User.where(user_id == "550e8400-e29b-41d4-a716-446655440000")
                .collect()
            "#,
        )
        .unwrap();
    assert_eq!(plan.nodes, plan_where.nodes);
    let rows = db.execute_plan(&plan).unwrap();
    let rows_where = db.execute_plan(&plan_where).unwrap();
    assert_eq!(rows, rows_where);
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].get("user_id"),
        Some(&Value::Str("550e8400-e29b-41d4-a716-446655440000".into()))
    );

    let plan = Planner::new(MysqlSource::capabilities())
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
                (
                    "user_id".into(),
                    Value::Str("6ba7b810-9dad-11d1-80b4-00c04fd430c8".into()),
                ),
                ("active".into(), Value::Bool(true)),
            ]),
        })
        .unwrap();
    assert!(n >= 1);
    let n = db
        .delete(
            "User",
            "user_id",
            &Value::Str("550e8400-e29b-41d4-a716-446655440000".into()),
        )
        .unwrap();
    assert!(n >= 1);

    let again = db.plan_managed_push(USER_SCHEMA).unwrap();
    assert!(again.changes.is_empty());

    let manifest = db.adopt(USER_SCHEMA).unwrap();
    assert_eq!(manifest.adapter_id, BACKEND_ID);

    db.transaction(|conn| {
        use mysql::prelude::*;
        conn.query_drop("DROP TABLE IF EXISTS `User`")?;
        Ok(())
    })
    .unwrap();
}

#[test]
fn live_same_connection_txn_visible_and_rolls_back_on_err() {
    let Some(url) = live_url() else {
        eprintln!("skip: set IRIS_TEST_MYSQL_URL for live MySQL conformance");
        return;
    };
    let db = MysqlSource::connect(&url)
        .expect("connect")
        .with_vos_schema(USER_SCHEMA)
        .expect("uuid schema map");

    db.transaction(|conn| {
        use mysql::prelude::*;
        conn.query_drop("DROP TABLE IF EXISTS `User`")?;
        Ok(())
    })
    .unwrap();
    db.managed_push(USER_SCHEMA).expect("push");

    let fixture_id = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
    let write = RowWrite {
        table: "User".into(),
        primary_key: "user_id".into(),
        fields: BTreeMap::from([
            ("user_id".into(), Value::Str(fixture_id.into())),
            ("user_name".into(), Value::Str("txn_visible".into())),
            ("active".into(), Value::Bool(true)),
        ]),
    };
    let plan = Planner::new(MysqlSource::capabilities())
        .plan_source(&format!(
            r#"User.filter(x => x.user_id == "{fixture_id}").collect()"#
        ))
        .unwrap();

    // Same connection: insert_on then execute_plan_on must see the row before commit.
    db.transaction(|conn| {
        db.insert_on(conn, &write)?;
        let rows = db.execute_plan_on(conn, &plan)?;
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].get("user_name"),
            Some(&Value::Str("txn_visible".into()))
        );
        Ok(())
    })
    .unwrap();

    // Committed row is visible on a fresh pool checkout.
    assert_eq!(db.execute_plan(&plan).unwrap().len(), 1);

    // Error path rolls back: insert_on then Err must leave no residue.
    let ghost_id = "bbbbbbbb-cccc-dddd-eeee-ffffffffffff";
    let ghost = RowWrite {
        table: "User".into(),
        primary_key: "user_id".into(),
        fields: BTreeMap::from([
            ("user_id".into(), Value::Str(ghost_id.into())),
            ("user_name".into(), Value::Str("ghost".into())),
            ("active".into(), Value::Bool(false)),
        ]),
    };
    let ghost_plan = Planner::new(MysqlSource::capabilities())
        .plan_source(&format!(
            r#"User.filter(x => x.user_id == "{ghost_id}").collect()"#
        ))
        .unwrap();
    let err = db
        .transaction(|conn| -> iris_adapter_mysql::Result<()> {
            db.insert_on(conn, &ghost)?;
            Err(iris_adapter_mysql::Error::Policy("force rollback".into()))
        })
        .expect_err("must roll back");
    assert!(err.to_string().contains("force rollback"));
    assert!(db.execute_plan(&ghost_plan).unwrap().is_empty());

    db.transaction(|conn| {
        use mysql::prelude::*;
        conn.query_drop("DROP TABLE IF EXISTS `User`")?;
        Ok(())
    })
    .unwrap();
}

#[test]
fn live_with_rollback_leaves_no_residue_on_success() {
    let Some(url) = live_url() else {
        eprintln!("skip: set IRIS_TEST_MYSQL_URL for live MySQL conformance");
        return;
    };
    let db = MysqlSource::connect(&url)
        .expect("connect")
        .with_vos_schema(USER_SCHEMA)
        .expect("uuid schema map");

    db.transaction(|conn| {
        use mysql::prelude::*;
        conn.query_drop("DROP TABLE IF EXISTS `User`")?;
        Ok(())
    })
    .unwrap();
    db.managed_push(USER_SCHEMA).expect("push");

    let fixture_id = "cccccccc-dddd-eeee-ffff-000000000000";
    let write = RowWrite {
        table: "User".into(),
        primary_key: "user_id".into(),
        fields: BTreeMap::from([
            ("user_id".into(), Value::Str(fixture_id.into())),
            ("user_name".into(), Value::Str("rollback_ok".into())),
            ("active".into(), Value::Bool(true)),
        ]),
    };
    let plan = Planner::new(MysqlSource::capabilities())
        .plan_source(&format!(
            r#"User.filter(x => x.user_id == "{fixture_id}").collect()"#
        ))
        .unwrap();

    db.with_rollback(|conn| {
        db.insert_on(conn, &write)?;
        let rows = db.execute_plan_on(conn, &plan)?;
        assert_eq!(rows.len(), 1);
        Ok(())
    })
    .unwrap();

    // Always ROLLBACK — even on Ok — so the shared DB stays clean.
    assert!(db.execute_plan(&plan).unwrap().is_empty());

    db.transaction(|conn| {
        use mysql::prelude::*;
        conn.query_drop("DROP TABLE IF EXISTS `User`")?;
        Ok(())
    })
    .unwrap();
}
