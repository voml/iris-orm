//! Redis as Composite Cache: AppliedWatermark, invalidate, apply outbox (Phase 10-C).
//!
//! Redis is never write authority. Values here are discardable projections;
//! freshness is proven only via watermarks comparable to authority CommitTokens.

use iris_ir::{AppliedWatermark, CommitToken, ConsistencyIntent, OutboxEffect, OutboxRecord};
use iris_types::{AppliedWatermarkState, CacheReadAction, CacheReadContext, decide_cache_read};
use redis::Commands;
use redis::Connection;
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

const WM_KEY_PREFIX: &str = "iris:wm:";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WatermarkWire {
    shard: String,
    seq: u64,
    applied_unix_ms: u64,
}

impl From<&AppliedWatermarkState> for WatermarkWire {
    fn from(s: &AppliedWatermarkState) -> Self {
        Self {
            shard: s.watermark.shard.clone(),
            seq: s.watermark.seq,
            applied_unix_ms: s.applied_unix_ms,
        }
    }
}

impl From<WatermarkWire> for AppliedWatermarkState {
    fn from(w: WatermarkWire) -> Self {
        Self {
            watermark: AppliedWatermark {
                shard: w.shard,
                seq: w.seq,
            },
            applied_unix_ms: w.applied_unix_ms,
        }
    }
}

/// Envelope stored alongside a cache payload (entity revision for projector idempotency).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheEntry {
    /// Authority-assigned entity version from the outbox record.
    pub entity_version: u64,
    /// Opaque payload (encoding validated by keyspace mapping).
    pub payload: String,
    /// Commit seq that produced this entry (informational).
    pub at_seq: u64,
}

fn wm_key(shard: &str) -> String {
    format!("{WM_KEY_PREFIX}{shard}")
}

pub(crate) fn get_watermark(
    conn: &mut Connection,
    shard: &str,
) -> Result<Option<AppliedWatermarkState>> {
    let key = wm_key(shard);
    let raw: Option<String> = conn.get(&key).map_err(Error::Redis)?;
    match raw {
        None => Ok(None),
        Some(s) => {
            let wire: WatermarkWire = serde_json::from_str(&s)
                .map_err(|e| Error::Policy(format!("corrupt cache watermark at `{key}`: {e}")))?;
            Ok(Some(wire.into()))
        }
    }
}

pub(crate) fn set_watermark(conn: &mut Connection, state: &AppliedWatermarkState) -> Result<()> {
    let key = wm_key(&state.watermark.shard);
    let wire = WatermarkWire::from(state);
    let raw = serde_json::to_string(&wire).map_err(|e| Error::Policy(e.to_string()))?;
    let _: () = conn.set(&key, raw).map_err(Error::Redis)?;
    Ok(())
}

/// Advance watermark monotonically (same shard only). Returns the stored state.
pub(crate) fn advance_watermark(
    conn: &mut Connection,
    token: &CommitToken,
    applied_unix_ms: u64,
) -> Result<AppliedWatermarkState> {
    let current = get_watermark(conn, &token.shard)?;
    let next_seq = match &current {
        Some(c) if c.watermark.shard == token.shard => c.watermark.seq.max(token.seq),
        Some(_) => {
            return Err(Error::Policy(
                "cache watermark shard mismatch during advance".into(),
            ));
        }
        None => token.seq,
    };
    let applied_unix_ms = match &current {
        Some(c) if next_seq == c.watermark.seq => c.applied_unix_ms.max(applied_unix_ms),
        _ => applied_unix_ms,
    };
    let state = AppliedWatermarkState {
        watermark: AppliedWatermark {
            shard: token.shard.clone(),
            seq: next_seq,
        },
        applied_unix_ms,
    };
    set_watermark(conn, &state)?;
    Ok(state)
}

fn entry_key(prefix: &str, primary_key: &str) -> String {
    // Separate from raw keyspace values: cache envelopes live under iris:cache:entry:
    // but still respect the mapping prefix for isolation.
    format!("{prefix}__cache_entry__{primary_key}")
}

pub(crate) fn get_entry(
    conn: &mut Connection,
    key_prefix: &str,
    primary_key: &str,
) -> Result<Option<CacheEntry>> {
    let key = entry_key(key_prefix, primary_key);
    let raw: Option<String> = conn.get(&key).map_err(Error::Redis)?;
    match raw {
        None => Ok(None),
        Some(s) => {
            let entry: CacheEntry = serde_json::from_str(&s)
                .map_err(|e| Error::Policy(format!("corrupt cache entry at `{key}`: {e}")))?;
            Ok(Some(entry))
        }
    }
}

pub(crate) fn put_entry(
    conn: &mut Connection,
    key_prefix: &str,
    primary_key: &str,
    entry: &CacheEntry,
    ttl_secs: Option<u64>,
) -> Result<()> {
    let key = entry_key(key_prefix, primary_key);
    let raw = serde_json::to_string(entry).map_err(|e| Error::Policy(e.to_string()))?;
    match ttl_secs {
        Some(ttl) => {
            let _: () = conn.set_ex(&key, raw, ttl).map_err(Error::Redis)?;
        }
        None => {
            let _: () = conn.set(&key, raw).map_err(Error::Redis)?;
        }
    }
    Ok(())
}

pub(crate) fn invalidate_entry(
    conn: &mut Connection,
    key_prefix: &str,
    primary_key: &str,
) -> Result<bool> {
    let key = entry_key(key_prefix, primary_key);
    let n: i64 = conn.del(&key).map_err(Error::Redis)?;
    Ok(n > 0)
}

/// Apply one durable outbox record into the cache projection (at-least-once, idempotent).
pub(crate) fn apply_outbox(
    conn: &mut Connection,
    key_prefix: &str,
    ttl_secs: Option<u64>,
    record: &OutboxRecord,
    payload: Option<&str>,
    now_unix_ms: u64,
) -> Result<AppliedWatermarkState> {
    match record.effect {
        OutboxEffect::Upsert => {
            let payload = payload.ok_or_else(|| {
                Error::Policy("cache upsert from outbox requires a payload".into())
            })?;
            let skip = get_entry(conn, key_prefix, &record.entity_id)?
                .is_some_and(|existing| existing.entity_version > record.entity_version);
            if !skip {
                put_entry(
                    conn,
                    key_prefix,
                    &record.entity_id,
                    &CacheEntry {
                        entity_version: record.entity_version,
                        payload: payload.to_string(),
                        at_seq: record.commit_token.seq,
                    },
                    ttl_secs,
                )?;
            }
        }
        OutboxEffect::Delete => {
            let skip = get_entry(conn, key_prefix, &record.entity_id)?
                .is_some_and(|existing| existing.entity_version > record.entity_version);
            if !skip {
                let _ = invalidate_entry(conn, key_prefix, &record.entity_id)?;
            }
        }
    }
    advance_watermark(conn, &record.commit_token, now_unix_ms)
}

/// Identity read through cache with consistency intent (hit or bypass signal).
pub(crate) fn identity_cache_read(
    conn: &mut Connection,
    key_prefix: &str,
    primary_key: &str,
    intent: &ConsistencyIntent,
    session_fence: Option<&CommitToken>,
    now_unix_ms: u64,
    shard: &str,
) -> Result<IdentityCacheResult> {
    let wm = get_watermark(conn, shard)?;
    let reachable = true; // connection succeeded
    let action = decide_cache_read(&CacheReadContext {
        intent,
        cache_wm: wm.as_ref(),
        session_fence,
        now_unix_ms,
        cache_reachable: reachable,
    });
    match action {
        CacheReadAction::UseCache { freshness_proven } => {
            let entry = get_entry(conn, key_prefix, primary_key)?;
            Ok(IdentityCacheResult::Hit {
                entry,
                watermark: wm,
                freshness_proven,
            })
        }
        CacheReadAction::BypassAuthority { reason } => Ok(IdentityCacheResult::BypassAuthority {
            reason,
            watermark: wm,
        }),
        CacheReadAction::FailClosed { reason } => Ok(IdentityCacheResult::FailClosed { reason }),
    }
}

/// Result of a cache-mediated identity read attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityCacheResult {
    /// Cache may serve this read (`entry` may still be `None` = miss ???fill).
    Hit {
        /// Cached body if present.
        entry: Option<CacheEntry>,
        /// Current watermark.
        watermark: Option<AppliedWatermarkState>,
        /// Whether freshness was proven for the intent.
        freshness_proven: bool,
    },
    /// Coordinator must read Authority (and optionally fill under stampede budget).
    BypassAuthority {
        /// Stable reason token.
        reason: &'static str,
        /// Watermark observed (if any).
        watermark: Option<AppliedWatermarkState>,
    },
    /// Fail closed per ProjectionRequired / policy.
    FailClosed {
        /// Stable reason token.
        reason: &'static str,
    },
}

/// Fill cache from an authority-sourced value after a bypass/miss.
pub(crate) fn fill_from_authority(
    conn: &mut Connection,
    key_prefix: &str,
    primary_key: &str,
    entry: &CacheEntry,
    ttl_secs: Option<u64>,
    token: &CommitToken,
    now_unix_ms: u64,
) -> Result<AppliedWatermarkState> {
    put_entry(conn, key_prefix, primary_key, entry, ttl_secs)?;
    // Fill does not invent a higher watermark than authority evidence provided.
    advance_watermark(conn, token, now_unix_ms)
}
