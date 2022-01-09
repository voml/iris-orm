//! Phase 10-B: Authority CommitToken + transactional outbox (SQLite).

use std::collections::BTreeMap;

use iris_adapter_sqlite::SqliteSource;
use iris_ir::{AppliedWatermark, CommitToken, OutboxAppend, OutboxEffect};
use iris_types::{RowWrite, Value};

const USER_SCHEMA: &str = r#"
table User {
    @@user_id: utf8,
    @user_name: utf8,
}
"#;

#[test]
fn authority_commit_appends_outbox_atomically_and_bumps_token() {
    let db = SqliteSource::open_in_memory().unwrap();
    db.managed_push(USER_SCHEMA).unwrap();
    db.ensure_authority_outbox().unwrap();
    assert_eq!(db.current_commit_token().unwrap(), CommitToken::new(0));

    let ((), token) = db
        .authority_commit(|txn| {
            txn.insert(&RowWrite {
                table: "User".into(),
                primary_key: "user_id".into(),
                fields: BTreeMap::from([
                    ("user_id".into(), Value::Str("u1".into())),
                    ("user_name".into(), Value::Str("alice".into())),
                ]),
            })?;
            txn.append_outbox(OutboxAppend {
                operation_id: "op-1".into(),
                table: "User".into(),
                entity_id: "u1".into(),
                entity_version: 1,
                effect: OutboxEffect::Upsert,
            });
            Ok(())
        })
        .unwrap();

    assert_eq!(token, CommitToken::new(1));
    assert_eq!(db.current_commit_token().unwrap(), token);
    assert!(token.is_covered_by(&AppliedWatermark::new(1)));
    assert!(!token.is_covered_by(&AppliedWatermark::new(0)));

    let pending = db.outbox_after(0, 10).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].operation_id, "op-1");
    assert_eq!(pending[0].commit_token, token);
    assert_eq!(pending[0].effect, OutboxEffect::Upsert);
    assert_eq!(db.outbox_backlog().unwrap(), 1);

    // Meta tables stay out of business catalog.
    let catalog = db.inspect().unwrap();
    assert!(catalog.table("User").is_some());
    assert!(catalog.table("_iris_outbox").is_none());
    assert!(catalog.table("_iris_commit").is_none());
}

#[test]
fn rolled_back_authority_write_leaves_no_outbox_or_token_bump() {
    let db = SqliteSource::open_in_memory().unwrap();
    db.managed_push(USER_SCHEMA).unwrap();
    db.ensure_authority_outbox().unwrap();

    let err = db.authority_commit(|txn| -> iris_adapter_sqlite::Result<()> {
        txn.insert(&RowWrite {
            table: "User".into(),
            primary_key: "user_id".into(),
            fields: BTreeMap::from([
                ("user_id".into(), Value::Str("u1".into())),
                ("user_name".into(), Value::Str("alice".into())),
            ]),
        })?;
        txn.append_outbox(OutboxAppend {
            operation_id: "op-rollback".into(),
            table: "User".into(),
            entity_id: "u1".into(),
            entity_version: 1,
            effect: OutboxEffect::Upsert,
        });
        Err(iris_adapter_sqlite::Error::Policy("forced rollback".into()))
    });
    assert!(err.is_err());
    assert_eq!(db.current_commit_token().unwrap(), CommitToken::new(0));
    assert!(db.outbox_after(0, 10).unwrap().is_empty());
    assert_eq!(db.outbox_backlog().unwrap(), 0);
}

#[test]
fn commit_token_monotonic_across_commits() {
    let db = SqliteSource::open_in_memory().unwrap();
    db.managed_push(USER_SCHEMA).unwrap();
    let (_, t1) = db
        .authority_commit(|txn| {
            txn.append_outbox(OutboxAppend {
                operation_id: "a".into(),
                table: "User".into(),
                entity_id: "1".into(),
                entity_version: 1,
                effect: OutboxEffect::Upsert,
            });
            Ok(())
        })
        .unwrap();
    let (_, t2) = db
        .authority_commit(|txn| {
            txn.append_outbox(OutboxAppend {
                operation_id: "b".into(),
                table: "User".into(),
                entity_id: "2".into(),
                entity_version: 1,
                effect: OutboxEffect::Delete,
            });
            Ok(())
        })
        .unwrap();
    assert_eq!(t1.seq, 1);
    assert_eq!(t2.seq, 2);
    assert_eq!(db.outbox_after(1, 10).unwrap().len(), 1);
    assert!(!format!("{t2:?}").contains("CREATE TABLE"));
}
