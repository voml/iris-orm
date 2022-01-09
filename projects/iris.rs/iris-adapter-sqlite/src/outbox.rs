//! Authority CommitToken + transactional outbox (Phase 10-B).
//!
//! SQL for meta tables stays **private**. Public values are Iris commit/outbox shapes.

use iris_ir::{CommitToken, DEFAULT_COMMIT_SHARD, OutboxAppend, OutboxEffect, OutboxRecord};
use iris_types::{RowWrite, Value};
use rusqlite::{Connection, OptionalExtension, Transaction};

use crate::execute;
use crate::{Error, Result};

/// True when a physical table is Iris authority meta (hidden from business catalog).
pub(crate) fn is_meta_table(name: &str) -> bool {
    name.starts_with("_iris_")
}

/// Idempotent install of commit counter + outbox tables.
pub fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _iris_commit (
            shard TEXT PRIMARY KEY NOT NULL,
            seq INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS _iris_outbox (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            shard TEXT NOT NULL,
            seq INTEGER NOT NULL,
            operation_id TEXT NOT NULL,
            table_name TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            entity_version INTEGER NOT NULL,
            effect TEXT NOT NULL,
            UNIQUE(operation_id, entity_version)
         );
         CREATE INDEX IF NOT EXISTS _iris_outbox_shard_seq
           ON _iris_outbox(shard, seq);",
    )?;
    let exists: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM _iris_commit WHERE shard = ?1",
            [DEFAULT_COMMIT_SHARD],
            |r| r.get(0),
        )
        .optional()?;
    if exists.is_none() {
        conn.execute(
            "INSERT INTO _iris_commit(shard, seq) VALUES (?1, 0)",
            [DEFAULT_COMMIT_SHARD],
        )?;
    }
    Ok(())
}

/// Read the current commit token (seq of last successful authority commit).
pub fn current_token(conn: &Connection) -> Result<CommitToken> {
    ensure_schema(conn)?;
    let seq: i64 = conn.query_row(
        "SELECT seq FROM _iris_commit WHERE shard = ?1",
        [DEFAULT_COMMIT_SHARD],
        |r| r.get(0),
    )?;
    Ok(CommitToken::new(seq as u64))
}

/// Writer handle bound to an open authority transaction.
pub struct AuthorityTxn<'conn> {
    tx: Transaction<'conn>,
    appends: Vec<OutboxAppend>,
}

impl<'conn> AuthorityTxn<'conn> {
    /// Queue an outbox append (flushed at commit with the new token).
    pub fn append_outbox(&mut self, event: OutboxAppend) {
        self.appends.push(event);
    }

    /// Insert a business row inside the authority transaction.
    pub fn insert(&self, write: &RowWrite) -> Result<()> {
        execute::insert_row(&self.tx, write)
    }

    /// Update by primary key inside the authority transaction.
    pub fn update(&self, write: &RowWrite) -> Result<usize> {
        execute::update_row(&self.tx, write)
    }

    /// Delete by primary key inside the authority transaction.
    pub fn delete(&self, table: &str, primary_key: &str, key: &Value) -> Result<usize> {
        execute::delete_row(&self.tx, table, primary_key, key)
    }
}

/// Run `f` inside one authority transaction: mutations + outbox appends are atomic.
///
/// On success, returns the user value and a [`CommitToken`] meaning authority committed
/// **and** durable outbox accepted propagation duty (not that projections caught up).
pub fn authority_commit<R>(
    conn: &mut Connection,
    f: impl FnOnce(&mut AuthorityTxn<'_>) -> Result<R>,
) -> Result<(R, CommitToken)> {
    ensure_schema(conn)?;
    let tx = conn.transaction()?;
    let mut writer = AuthorityTxn {
        tx,
        appends: Vec::new(),
    };
    let value = f(&mut writer)?;
    let appends = std::mem::take(&mut writer.appends);

    let prev: i64 = writer.tx.query_row(
        "SELECT seq FROM _iris_commit WHERE shard = ?1",
        [DEFAULT_COMMIT_SHARD],
        |r| r.get(0),
    )?;
    let next = (prev as u64).saturating_add(1);
    writer.tx.execute(
        "UPDATE _iris_commit SET seq = ?1 WHERE shard = ?2",
        rusqlite::params![next as i64, DEFAULT_COMMIT_SHARD],
    )?;

    for ev in &appends {
        let effect = match ev.effect {
            OutboxEffect::Upsert => "upsert",
            OutboxEffect::Delete => "delete",
        };
        writer.tx.execute(
            "INSERT INTO _iris_outbox(
                shard, seq, operation_id, table_name, entity_id, entity_version, effect
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                DEFAULT_COMMIT_SHARD,
                next as i64,
                ev.operation_id,
                ev.table,
                ev.entity_id,
                ev.entity_version as i64,
                effect,
            ],
        )?;
    }

    writer.tx.commit()?;
    Ok((value, CommitToken::new(next)))
}

/// List durable outbox records with `seq > after_seq` (projector poll shape).
pub fn outbox_after(conn: &Connection, after_seq: u64, limit: usize) -> Result<Vec<OutboxRecord>> {
    ensure_schema(conn)?;
    let mut stmt = conn.prepare(
        "SELECT id, shard, seq, operation_id, table_name, entity_id, entity_version, effect
         FROM _iris_outbox
         WHERE shard = ?1 AND seq > ?2
         ORDER BY seq ASC, id ASC
         LIMIT ?3",
    )?;
    let rows = stmt.query_map(
        rusqlite::params![DEFAULT_COMMIT_SHARD, after_seq as i64, limit as i64],
        |row| {
            let effect_raw: String = row.get(7)?;
            let effect = match effect_raw.as_str() {
                "delete" => OutboxEffect::Delete,
                _ => OutboxEffect::Upsert,
            };
            let shard: String = row.get(1)?;
            let seq: i64 = row.get(2)?;
            Ok(OutboxRecord {
                id: row.get::<_, i64>(0)? as u64,
                commit_token: CommitToken {
                    shard,
                    seq: seq as u64,
                },
                operation_id: row.get(3)?,
                table: row.get(4)?,
                entity_id: row.get(5)?,
                entity_version: row.get::<_, i64>(6)? as u64,
                effect,
            })
        },
    )?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(Error::from)?);
    }
    Ok(out)
}

/// Count pending outbox rows (backlog observability).
pub fn outbox_backlog(conn: &Connection) -> Result<u64> {
    ensure_schema(conn)?;
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM _iris_outbox", [], |r| r.get(0))?;
    Ok(n as u64)
}
