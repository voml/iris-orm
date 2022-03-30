//! Phase 10-B: Authority CommitToken + outbox (PostgreSQL, env-gated).

use std::collections::BTreeMap;

use iris_adapter_postgres::PostgresSource;
use iris_ir::{CommitToken, OutboxAppend, OutboxEffect};
use iris_types::{RowWrite, Value};

fn live_url() -> Option<String> {
    std::env::var("IRIS_TEST_POSTGRES_URL")
        .ok()
        .filter(|s| !s.is_empty())
}

const SCHEMA: &str = r#"
table IrisOutboxUser {
    @@user_id: utf8,
    @user_name: utf8,
}
"#;

#[test]
fn live_authority_outbox_commit_token() {
    let Some(url) = live_url() else {
        eprintln!("skip: set IRIS_TEST_POSTGRES_URL for Phase 10-B Postgres outbox");
        return;
    };
    let db = PostgresSource::connect(&url).expect("connect");
    db.with_connection(|c| {
        c.batch_execute(
            "DROP TABLE IF EXISTS \"IrisOutboxUser\";
             DROP TABLE IF EXISTS _iris_outbox;
             DROP TABLE IF EXISTS _iris_commit;",
        )?;
        Ok(())
    })
    .unwrap();

    db.managed_push(SCHEMA).unwrap();
    db.ensure_authority_outbox().unwrap();
    assert_eq!(db.current_commit_token().unwrap(), CommitToken::new(0));

    let ((), token) = db
        .authority_commit(|txn| {
            txn.insert(&RowWrite {
                table: "IrisOutboxUser".into(),
                primary_key: "user_id".into(),
                fields: BTreeMap::from([
                    ("user_id".into(), Value::Str("u1".into())),
                    ("user_name".into(), Value::Str("alice".into())),
                ]),
            })?;
            txn.append_outbox(OutboxAppend {
                operation_id: "pg-op-1".into(),
                table: "IrisOutboxUser".into(),
                entity_id: "u1".into(),
                entity_version: 1,
                effect: OutboxEffect::Upsert,
            });
            Ok(())
        })
        .unwrap();

    assert_eq!(token.seq, 1);
    let pending = db.outbox_after(0, 10).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].operation_id, "pg-op-1");
    assert!(db.inspect().unwrap().table("_iris_outbox").is_none());

    db.with_connection(|c| {
        c.batch_execute(
            "DROP TABLE IF EXISTS \"IrisOutboxUser\";
             DROP TABLE IF EXISTS _iris_outbox;
             DROP TABLE IF EXISTS _iris_commit;",
        )?;
        Ok(())
    })
    .unwrap();
}
