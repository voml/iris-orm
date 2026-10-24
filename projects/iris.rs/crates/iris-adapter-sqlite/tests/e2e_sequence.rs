//! End-to-end SQLite sequence: inspect, push, generated operation, transaction,
//! rollback, reopen, and catalog cross-check against the raw engine.

use std::collections::BTreeMap;

use iris_adapter_sqlite::{RusqliteProvider, SqliteSource};
use iris_types::{Planner, RowWrite, Value};

const USER_SCHEMA: &str = r#"
table User {
    @@user_id: utf8,
    @user_name: utf8,
    active: bool,
}
"#;

fn temp_path(label: &str) -> std::path::PathBuf {
    std::env::temp_dir()
        .join(format!("iris-sqlite-e2e-{label}-{}.db", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()))
}

fn remove_database(path: &std::path::Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn generated_operation_survives_reopen_with_rollback_and_catalog_cross_check() {
    let path = temp_path("sequence");

    // 1. inspect before push: no physical table yet, drift is not clean.
    {
        let db = SqliteSource::open(&path).unwrap();
        assert!(db.inspect().unwrap().table("User").is_none());
        assert!(!db.drift(USER_SCHEMA).unwrap().is_clean());
    }

    // 2. managed push: review the plan, then apply the same plan.
    {
        let db = SqliteSource::open(&path).unwrap();
        let plan = db.plan_managed_push(USER_SCHEMA).unwrap();
        assert_eq!(plan.changes.len(), 1);
        assert!(!plan.destructive);
        db.apply_managed_push(&plan, USER_SCHEMA).unwrap();
        assert!(db.drift(USER_SCHEMA).unwrap().is_clean());
    }

    let planner = Planner::new(SqliteSource::capabilities());

    // 3. generated operation INSERT: VOS source becomes a physical plan.
    {
        let db = SqliteSource::open(&path).unwrap();
        let insert = planner.plan_source(r#"User::insert({ user_id: "e2e", user_name: "persist", active: true })"#).unwrap();
        let inserted = db.execute_plan(&insert).unwrap();
        assert_eq!(inserted.len(), 1);
        assert_eq!(inserted[0].get("user_id"), Some(&Value::Str("e2e".into())));
        assert_eq!(inserted[0].get("user_name"), Some(&Value::Str("persist".into())));
        assert_eq!(inserted[0].get("active"), Some(&Value::Int(1)));

        // 4. generated operation SELECT reads the committed row back.
        let select = planner.plan_source(r#"User.filter(x => x.user_id == "e2e").collect()"#).unwrap();
        let selected = db.execute_plan(&select).unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].get("user_name"), Some(&Value::Str("persist".into())));
    }

    // 5. transaction then rollback discards the uncommitted write.
    {
        let db = SqliteSource::open(&path).unwrap();
        db.begin().unwrap();
        db.insert(&RowWrite {
            table: "User".into(),
            primary_key: "user_id".into(),
            fields: BTreeMap::from([
                ("user_id".into(), Value::Str("ghost".into())),
                ("user_name".into(), Value::Str("rollback".into())),
                ("active".into(), Value::Bool(false)),
            ]),
        })
        .unwrap();
        db.rollback().unwrap();

        let ghost = planner.plan_source(r#"User.filter(x => x.user_id == "ghost").collect()"#).unwrap();
        assert!(db.execute_plan(&ghost).unwrap().is_empty());
    }

    // 6. reopen: committed row persists, rolled back row stays gone.
    {
        let db = SqliteSource::open(&path).unwrap();
        let persisted = planner.plan_source(r#"User.filter(x => x.user_id == "e2e").collect()"#).unwrap();
        let rows = db.execute_plan(&persisted).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("user_name"), Some(&Value::Str("persist".into())));

        let ghost = planner.plan_source(r#"User.filter(x => x.user_id == "ghost").collect()"#).unwrap();
        assert!(db.execute_plan(&ghost).unwrap().is_empty());

        // 7. migration and catalog cross-check through the raw engine.
        assert!(db.drift(USER_SCHEMA).unwrap().is_clean());
        let raw = RusqliteProvider::open(&path).unwrap();
        let catalog = raw.inspect_catalog().unwrap();
        let user = catalog.table("User").expect("User table");
        let names: Vec<&str> = user.columns.iter().map(|column| column.name.as_str()).collect();
        assert_eq!(names, vec!["user_id", "user_name", "active"]);
        assert!(user.columns[0].primary_key);
        assert!(!user.columns[1].primary_key);
        let physical_tables: Vec<&str> = catalog.tables.iter().map(|table| table.name.as_str()).collect();
        assert_eq!(physical_tables, vec!["User"]);

        let rows = raw
            .execute_one("SELECT user_name FROM \"User\" WHERE user_id = ?1", &[iris_adapter_sqlite::SqliteValue::Text(b"e2e".to_vec())])
            .unwrap();
        assert_eq!(rows.rows[0][0], iris_adapter_sqlite::SqliteValue::Text(b"persist".to_vec()));
    }

    remove_database(&path);
}
