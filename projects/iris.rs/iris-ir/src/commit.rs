//! Authority commit tokens and outbox event shapes (Phase 10-B).
//!
//! Backend-neutral types only. Physical outbox DDL stays private inside adapters.

use serde::{Deserialize, Serialize};

/// Default shard / partition label for a single-node authority.
pub const DEFAULT_COMMIT_SHARD: &str = "default";

/// Monotonic authority commit position (freshness proof).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitToken {
    /// Shard / partition id (Phase 10-B: usually [`DEFAULT_COMMIT_SHARD`]).
    pub shard: String,
    /// Monotonic sequence within the shard.
    pub seq: u64,
}

impl CommitToken {
    /// Build a token for the default shard.
    pub fn new(seq: u64) -> Self {
        Self {
            shard: DEFAULT_COMMIT_SHARD.into(),
            seq,
        }
    }

    /// True when `other` is the same shard and at least as new.
    pub fn is_covered_by(&self, watermark: &AppliedWatermark) -> bool {
        self.shard == watermark.shard && watermark.seq >= self.seq
    }
}

/// Projection / cache applied watermark (comparable to [`CommitToken`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedWatermark {
    /// Shard / partition id.
    pub shard: String,
    /// Highest authority seq applied by the projection.
    pub seq: u64,
}

impl AppliedWatermark {
    /// Watermark for the default shard.
    pub fn new(seq: u64) -> Self {
        Self {
            shard: DEFAULT_COMMIT_SHARD.into(),
            seq,
        }
    }
}

/// Logical outbox effect kind (not a store command).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboxEffect {
    /// Entity upserted / inserted.
    Upsert,
    /// Entity deleted.
    Delete,
}

/// Append request collected inside an authority transaction (pre-commit).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboxAppend {
    /// Idempotency key for projectors (`operation_id` + entity version).
    pub operation_id: String,
    /// VOS table name.
    pub table: String,
    /// Primary identity as utf-8 string.
    pub entity_id: String,
    /// Monotonic entity revision assigned by the authority write path.
    pub entity_version: u64,
    /// Effect kind.
    pub effect: OutboxEffect,
}

/// Durable outbox row after authority commit (propagation duty accepted).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboxRecord {
    /// Opaque durable id (adapter-assigned).
    pub id: u64,
    /// Commit token that includes this event.
    pub commit_token: CommitToken,
    /// Original append fields.
    pub operation_id: String,
    /// VOS table.
    pub table: String,
    /// Entity id.
    pub entity_id: String,
    /// Entity version.
    pub entity_version: u64,
    /// Effect.
    pub effect: OutboxEffect,
}
