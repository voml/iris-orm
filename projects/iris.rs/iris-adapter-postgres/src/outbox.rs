//! Authority CommitToken + transactional outbox (Phase 10-B, PostgreSQL).
//!
//! SQL stays **private**. Public values are Iris commit/outbox shapes.

use iris_ir::{CommitToken, DEFAULT_COMMIT_SHARD, OutboxAppend, OutboxEffect, OutboxRecord};
use iris_types::{RowWrite, Value};
use postgres::Client;
use postgres::Transaction;

use crate::Result;
use crate::execute;

/// True when a physical table is Iris authority meta.
pub(crate) fn is_meta_table(name: &str) -> bool {
    name.starts_with("_iris_")
}

/// Idempotent install of commit counter + outbox tables.
pub fn ensure_schema(client: &mut Client) -> Result<()> {
    client.batch_execute(
        "CREATE TABLE IF NOT EXISTS _iris_commit (
            shard TEXT PRIMARY KEY NOT NULL,
            seq BIGINT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS _iris_outbox (
            id BIGSERIAL PRIMARY KEY,
            shard TEXT NOT NULL,
            seq BIGINT NOT NULL,
            operation_id TEXT NOT NULL,
            table_name TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            entity_version BIGINT NOT NULL,
            effect TEXT NOT NULL,
            UNIQUE(operation_id, entity_version)
         );
         CREATE INDEX IF NOT EXISTS _iris_outbox_shard_seq
           ON _iris_outbox(shard, seq);",
    )?;
    let row = client.query_opt(
        "SELECT 1 FROM _iris_commit WHERE shard = $1",
        &[&DEFAULT_COMMIT_SHARD],
    )?;
    if row.is_none() {
        client.execute(
            "INSERT INTO _iris_commit(shard, seq) VALUES ($1, 0)",
            &[&DEFAULT_COMMIT_SHARD],
        )?;
    }
    Ok(())
}

/// Current commit token.
pub fn current_token(client: &mut Client) -> Result<CommitToken> {
    ensure_schema(client)?;
    let row = client.query_one(
        "SELECT seq FROM _iris_commit WHERE shard = $1",
        &[&DEFAULT_COMMIT_SHARD],
    )?;
    let seq: i64 = row.get(0);
    Ok(CommitToken::new(seq as u64))
}

/// Writer handle for an authority transaction.
pub struct AuthorityTxn<'a> {
    tx: Transaction<'a>,
    appends: Vec<OutboxAppend>,
}

impl<'a> AuthorityTxn<'a> {
    /// Queue outbox append.
    pub fn append_outbox(&mut self, event: OutboxAppend) {
        self.appends.push(event);
    }

    /// Insert inside the authority transaction.
    pub fn insert(&mut self, write: &RowWrite) -> Result<()> {
        execute::insert_row(&mut self.tx, write)
    }

    /// Update inside the authority transaction.
    pub fn update(&mut self, write: &RowWrite) -> Result<u64> {
        execute::update_row(&mut self.tx, write)
    }

    /// Delete inside the authority transaction.
    pub fn delete(&mut self, table: &str, primary_key: &str, key: &Value) -> Result<u64> {
        execute::delete_row(&mut self.tx, table, primary_key, key)
    }
}

/// Atomic authority mutations + outbox append; returns [`CommitToken`].
pub fn authority_commit<R>(
    client: &mut Client,
    f: impl FnOnce(&mut AuthorityTxn<'_>) -> Result<R>,
) -> Result<(R, CommitToken)> {
    ensure_schema(client)?;
    let tx = client.transaction()?;
    let mut writer = AuthorityTxn {
        tx,
        appends: Vec::new(),
    };
    let value = f(&mut writer)?;
    let appends = std::mem::take(&mut writer.appends);

    let prev: i64 = writer
        .tx
        .query_one(
            "SELECT seq FROM _iris_commit WHERE shard = $1",
            &[&DEFAULT_COMMIT_SHARD],
        )?
        .get(0);
    let next = (prev as u64).saturating_add(1);
    writer.tx.execute(
        "UPDATE _iris_commit SET seq = $1 WHERE shard = $2",
        &[&(next as i64), &DEFAULT_COMMIT_SHARD],
    )?;

    for ev in &appends {
        let effect = match ev.effect {
            OutboxEffect::Upsert => "upsert",
            OutboxEffect::Delete => "delete",
        };
        writer.tx.execute(
            "INSERT INTO _iris_outbox(
                shard, seq, operation_id, table_name, entity_id, entity_version, effect
             ) VALUES ($1, $2, $3, $4, $5, $6, $7)",
            &[
                &DEFAULT_COMMIT_SHARD,
                &(next as i64),
                &ev.operation_id,
                &ev.table,
                &ev.entity_id,
                &(ev.entity_version as i64),
                &effect,
            ],
        )?;
    }

    writer.tx.commit()?;
    Ok((value, CommitToken::new(next)))
}

/// Poll outbox after sequence.
pub fn outbox_after(
    client: &mut Client,
    after_seq: u64,
    limit: usize,
) -> Result<Vec<OutboxRecord>> {
    ensure_schema(client)?;
    let rows = client.query(
        "SELECT id, shard, seq, operation_id, table_name, entity_id, entity_version, effect
         FROM _iris_outbox
         WHERE shard = $1 AND seq > $2
         ORDER BY seq ASC, id ASC
         LIMIT $3",
        &[&DEFAULT_COMMIT_SHARD, &(after_seq as i64), &(limit as i64)],
    )?;
    let mut out = Vec::new();
    for row in rows {
        let effect_raw: String = row.get(7);
        let effect = match effect_raw.as_str() {
            "delete" => OutboxEffect::Delete,
            _ => OutboxEffect::Upsert,
        };
        let shard: String = row.get(1);
        let seq: i64 = row.get(2);
        out.push(OutboxRecord {
            id: row.get::<_, i64>(0) as u64,
            commit_token: CommitToken {
                shard,
                seq: seq as u64,
            },
            operation_id: row.get(3),
            table: row.get(4),
            entity_id: row.get(5),
            entity_version: row.get::<_, i64>(6) as u64,
            effect,
        });
    }
    Ok(out)
}

/// Outbox backlog count.
pub fn outbox_backlog(client: &mut Client) -> Result<u64> {
    ensure_schema(client)?;
    let n: i64 = client
        .query_one("SELECT COUNT(*) FROM _iris_outbox", &[])?
        .get(0);
    Ok(n as u64)
}
