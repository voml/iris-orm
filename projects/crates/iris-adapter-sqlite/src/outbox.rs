//! Authority CommitToken + transactional outbox (Phase 10-B).
//!
//! SQL for meta tables stays **private**. Public values are Iris commit/outbox shapes.

use iris_ir::{CommitToken, DEFAULT_COMMIT_SHARD, OutboxAppend, OutboxEffect, OutboxRecord};
use iris_types::{RowWrite, Value};
use sqlite_provider::{SqliteProvider, SqliteValue};

use crate::{Error, Result, execute};

/// True when a physical table is Iris authority meta (hidden from business catalog).
pub(crate) fn is_meta_table(name: &str) -> bool {
    name.starts_with("_iris_")
}

fn shard_param() -> SqliteValue {
    SqliteValue::Text(DEFAULT_COMMIT_SHARD.as_bytes().to_vec())
}

fn text_param(value: &str) -> SqliteValue {
    SqliteValue::Text(value.as_bytes().to_vec())
}

fn integer_param(value: i64) -> SqliteValue {
    SqliteValue::Integer(value)
}

/// Idempotent install of commit counter + outbox tables.
pub fn ensure_schema(provider: &impl SqliteProvider) -> Result<()> {
    provider.execute_batch(
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
    let exists = provider.execute_one("SELECT 1 FROM _iris_commit WHERE shard = ?1", &[shard_param()])?;
    if exists.rows.is_empty() {
        provider.execute_one("INSERT INTO _iris_commit(shard, seq) VALUES (?1, 0)", &[shard_param()])?;
    }
    Ok(())
}

/// Read the current commit token (seq of last successful authority commit).
pub fn current_token(provider: &impl SqliteProvider) -> Result<CommitToken> {
    ensure_schema(provider)?;
    let result = provider.execute_one("SELECT seq FROM _iris_commit WHERE shard = ?1", &[shard_param()])?;
    let seq = result
        .rows
        .first()
        .and_then(|row| row.first())
        .and_then(|value| match value {
            SqliteValue::Integer(seq) => Some(*seq),
            _ => None,
        })
        .ok_or_else(|| Error::Policy("missing authority commit row".into()))?;
    Ok(CommitToken::new(seq as u64))
}

/// Writer handle bound to an open authority transaction.
pub struct AuthorityTxn<'provider, P: SqliteProvider> {
    provider: &'provider P,
    appends: Vec<OutboxAppend>,
}

impl<'provider, P: SqliteProvider> AuthorityTxn<'provider, P> {
    /// Queue an outbox append (flushed at commit with the new token).
    pub fn append_outbox(&mut self, event: OutboxAppend) {
        self.appends.push(event);
    }

    /// Insert a business row inside the authority transaction.
    pub fn insert(&self, write: &RowWrite) -> Result<()> {
        execute::insert_row(self.provider, write)
    }

    /// Update by primary key inside the authority transaction.
    pub fn update(&self, write: &RowWrite) -> Result<usize> {
        execute::update_row(self.provider, write)
    }

    /// Delete by primary key inside the authority transaction.
    pub fn delete(&self, table: &str, primary_key: &str, key: &Value) -> Result<usize> {
        execute::delete_row(self.provider, table, primary_key, key)
    }
}

/// Run `f` inside one authority transaction: mutations + outbox appends are atomic.
///
/// On success, returns the user value and a [`CommitToken`] meaning authority committed
/// **and** durable outbox accepted propagation duty (not that projections caught up).
pub fn authority_commit<P: SqliteProvider, R>(provider: &P, f: impl FnOnce(&mut AuthorityTxn<'_, P>) -> Result<R>) -> Result<(R, CommitToken)> {
    ensure_schema(provider)?;
    provider.begin_immediate()?;
    let committed = (|| {
        let mut writer = AuthorityTxn { provider, appends: Vec::new() };
        let value = f(&mut writer)?;
        let appends = std::mem::take(&mut writer.appends);

        let prev = provider.execute_one("SELECT seq FROM _iris_commit WHERE shard = ?1", &[shard_param()])?;
        let prev_seq = prev
            .rows
            .first()
            .and_then(|row| row.first())
            .and_then(|value| match value {
                SqliteValue::Integer(seq) => Some(*seq),
                _ => None,
            })
            .ok_or_else(|| Error::Policy("missing authority commit row".into()))?;
        let next = (prev_seq as u64).saturating_add(1);
        provider.execute_one(
            "UPDATE _iris_commit SET seq = ?1 WHERE shard = ?2",
            &[integer_param(next as i64), shard_param()],
        )?;

        for ev in &appends {
            let effect = match ev.effect {
                OutboxEffect::Upsert => "upsert",
                OutboxEffect::Delete => "delete",
            };
            provider.execute_one(
                "INSERT INTO _iris_outbox(
                    shard, seq, operation_id, table_name, entity_id, entity_version, effect
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                &[
                    shard_param(),
                    integer_param(next as i64),
                    text_param(&ev.operation_id),
                    text_param(&ev.table),
                    text_param(&ev.entity_id),
                    integer_param(ev.entity_version as i64),
                    text_param(effect),
                ],
            )?;
        }

        provider.commit()?;
        Ok((value, CommitToken::new(next)))
    })();
    if committed.is_err() {
        let _ = provider.rollback();
    }
    committed
}

/// List durable outbox records with `seq > after_seq` (projector poll shape).
pub fn outbox_after(provider: &impl SqliteProvider, after_seq: u64, limit: usize) -> Result<Vec<OutboxRecord>> {
    ensure_schema(provider)?;
    let result = provider.execute_one(
        "SELECT id, shard, seq, operation_id, table_name, entity_id, entity_version, effect
         FROM _iris_outbox
         WHERE shard = ?1 AND seq > ?2
         ORDER BY seq ASC, id ASC
         LIMIT ?3",
        &[shard_param(), integer_param(after_seq as i64), integer_param(limit as i64)],
    )?;

    let mut out = Vec::new();
    for row in result.rows {
        let effect_raw = row
            .get(7)
            .and_then(|value| match value {
                SqliteValue::Text(bytes) => Some(String::from_utf8_lossy(bytes).into_owned()),
                _ => None,
            })
            .ok_or_else(|| Error::Policy("invalid outbox effect".into()))?;
        let effect = match effect_raw.as_str() {
            "delete" => OutboxEffect::Delete,
            _ => OutboxEffect::Upsert,
        };
        let shard = row
            .get(1)
            .and_then(|value| match value {
                SqliteValue::Text(bytes) => Some(String::from_utf8_lossy(bytes).into_owned()),
                _ => None,
            })
            .ok_or_else(|| Error::Policy("invalid outbox shard".into()))?;
        let seq = row
            .get(2)
            .and_then(|value| match value {
                SqliteValue::Integer(seq) => Some(*seq),
                _ => None,
            })
            .ok_or_else(|| Error::Policy("invalid outbox seq".into()))?;
        let id = row
            .get(0)
            .and_then(|value| match value {
                SqliteValue::Integer(id) => Some(*id),
                _ => None,
            })
            .ok_or_else(|| Error::Policy("invalid outbox id".into()))?;
        let operation_id = row
            .get(3)
            .and_then(|value| match value {
                SqliteValue::Text(bytes) => Some(String::from_utf8_lossy(bytes).into_owned()),
                _ => None,
            })
            .ok_or_else(|| Error::Policy("invalid outbox operation_id".into()))?;
        let table = row
            .get(4)
            .and_then(|value| match value {
                SqliteValue::Text(bytes) => Some(String::from_utf8_lossy(bytes).into_owned()),
                _ => None,
            })
            .ok_or_else(|| Error::Policy("invalid outbox table".into()))?;
        let entity_id = row
            .get(5)
            .and_then(|value| match value {
                SqliteValue::Text(bytes) => Some(String::from_utf8_lossy(bytes).into_owned()),
                _ => None,
            })
            .ok_or_else(|| Error::Policy("invalid outbox entity_id".into()))?;
        let entity_version = row
            .get(6)
            .and_then(|value| match value {
                SqliteValue::Integer(version) => Some(*version),
                _ => None,
            })
            .ok_or_else(|| Error::Policy("invalid outbox entity_version".into()))?;

        out.push(OutboxRecord {
            id: id as u64,
            commit_token: CommitToken { shard, seq: seq as u64 },
            operation_id,
            table,
            entity_id,
            entity_version: entity_version as u64,
            effect,
        });
    }
    Ok(out)
}

/// Count pending outbox rows (backlog observability).
pub fn outbox_backlog(provider: &impl SqliteProvider) -> Result<u64> {
    ensure_schema(provider)?;
    let result = provider.execute_one("SELECT COUNT(*) FROM _iris_outbox", &[])?;
    let count = result
        .rows
        .first()
        .and_then(|row| row.first())
        .and_then(|value| match value {
            SqliteValue::Integer(count) => Some(*count),
            _ => None,
        })
        .ok_or_else(|| Error::Policy("invalid outbox backlog count".into()))?;
    Ok(count as u64)
}
