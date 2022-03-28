//! Cache freshness verdicts for Composite identity reads (Phase 10-C).
//!
//! Offline / pure logic: adapters supply watermarks; this module never speaks
//! Redis/SQL. Wall-clock alone cannot prove freshness -- an
//! [`AppliedWatermark`] must be present and comparable.

use iris_ir::{AppliedWatermark, CommitToken, ConsistencyIntent};
use std::sync::atomic::{AtomicU32, Ordering};

/// Projection watermark plus when it was applied (for BoundedStale lag proof).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedWatermarkState {
    /// Comparable watermark.
    pub watermark: AppliedWatermark,
    /// Unix epoch milliseconds when this watermark was last advanced.
    pub applied_unix_ms: u64,
}

impl AppliedWatermarkState {
    /// Construct state for the default shard.
    pub fn new(seq: u64, applied_unix_ms: u64) -> Self {
        Self {
            watermark: AppliedWatermark::new(seq),
            applied_unix_ms,
        }
    }
}

/// Inputs for deciding whether a cache hit may be returned.
#[derive(Debug, Clone)]
pub struct CacheReadContext<'a> {
    /// Application consistency intent.
    pub intent: &'a ConsistencyIntent,
    /// Latest cache-reported watermark, if known.
    pub cache_wm: Option<&'a AppliedWatermarkState>,
    /// Session fence from the last authority write (ReadYourWrites).
    pub session_fence: Option<&'a CommitToken>,
    /// Current wall time (ms); only used with a watermark for BoundedStale.
    pub now_unix_ms: u64,
    /// Whether the cache component is reachable.
    pub cache_reachable: bool,
}

/// What the coordinator should do next for an identity read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheReadAction {
    /// Serve from cache (freshness proven when `freshness_proven` is true).
    UseCache {
        /// True when watermark/fence proves the intent.
        freshness_proven: bool,
    },
    /// Do not use cache; read Authority (and optionally fill under stampede budget).
    BypassAuthority {
        /// Stable reason token (no secrets / private commands).
        reason: &'static str,
    },
    /// Fail closed (e.g. ProjectionRequired + cache down).
    FailClosed {
        /// Stable reason token.
        reason: &'static str,
    },
}

/// Decide cache vs authority for an identity/cacheable read.
pub fn decide_cache_read(ctx: &CacheReadContext<'_>) -> CacheReadAction {
    match ctx.intent {
        ConsistencyIntent::Authoritative => CacheReadAction::BypassAuthority {
            reason: "authoritative_requires_authority",
        },
        ConsistencyIntent::ReadYourWrites => decide_read_your_writes(ctx),
        ConsistencyIntent::BoundedStale { max_lag_secs } => {
            decide_bounded_stale(ctx, *max_lag_secs)
        }
        ConsistencyIntent::Eventual => {
            if !ctx.cache_reachable {
                return CacheReadAction::BypassAuthority {
                    reason: "cache_unreachable",
                };
            }
            CacheReadAction::UseCache {
                freshness_proven: ctx.cache_wm.is_some(),
            }
        }
        ConsistencyIntent::ProjectionRequired { .. } => {
            if !ctx.cache_reachable {
                return CacheReadAction::FailClosed {
                    reason: "projection_required_unreachable",
                };
            }
            if ctx.cache_wm.is_none() {
                return CacheReadAction::FailClosed {
                    reason: "projection_required_unknown_watermark",
                };
            }
            CacheReadAction::UseCache {
                freshness_proven: true,
            }
        }
    }
}

fn decide_read_your_writes(ctx: &CacheReadContext<'_>) -> CacheReadAction {
    if !ctx.cache_reachable {
        return CacheReadAction::BypassAuthority {
            reason: "cache_unreachable",
        };
    }
    let Some(fence) = ctx.session_fence else {
        // No session writes yet -- cache is acceptable without a fence.
        return CacheReadAction::UseCache {
            freshness_proven: ctx.cache_wm.is_some(),
        };
    };
    let Some(state) = ctx.cache_wm else {
        return CacheReadAction::BypassAuthority {
            reason: "ryw_unknown_watermark",
        };
    };
    if fence.is_covered_by(&state.watermark) {
        CacheReadAction::UseCache {
            freshness_proven: true,
        }
    } else {
        CacheReadAction::BypassAuthority {
            reason: "ryw_fence_not_covered",
        }
    }
}

fn decide_bounded_stale(ctx: &CacheReadContext<'_>, max_lag_secs: u64) -> CacheReadAction {
    if !ctx.cache_reachable {
        return CacheReadAction::BypassAuthority {
            reason: "cache_unreachable",
        };
    }
    let Some(state) = ctx.cache_wm else {
        // Unknown watermark cannot prove lag -- must not use cache.
        return CacheReadAction::BypassAuthority {
            reason: "bounded_stale_unknown_watermark",
        };
    };
    let max_lag_ms = max_lag_secs.saturating_mul(1000);
    let lag_ms = ctx.now_unix_ms.saturating_sub(state.applied_unix_ms);
    if lag_ms <= max_lag_ms {
        CacheReadAction::UseCache {
            freshness_proven: true,
        }
    } else {
        CacheReadAction::BypassAuthority {
            reason: "bounded_stale_lag_exceeded",
        }
    }
}

/// Process-local singleflight / stampede budget for authority fill-on-miss.
#[derive(Debug)]
pub struct StampedeBudget {
    budget: u32,
    in_flight: AtomicU32,
}

impl StampedeBudget {
    /// Create a budget (`0` means no concurrent fills allowed).
    pub fn new(budget: u32) -> Self {
        Self {
            budget,
            in_flight: AtomicU32::new(0),
        }
    }

    /// Configured concurrency limit.
    pub fn limit(&self) -> u32 {
        self.budget
    }

    /// Current in-flight fill count.
    pub fn in_flight(&self) -> u32 {
        self.in_flight.load(Ordering::Relaxed)
    }

    /// Try to acquire a fill slot. Returns `false` when the budget is exhausted.
    pub fn try_acquire(&self) -> bool {
        loop {
            let cur = self.in_flight.load(Ordering::Acquire);
            if cur >= self.budget {
                return false;
            }
            if self
                .in_flight
                .compare_exchange(cur, cur + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return true;
            }
        }
    }

    /// Release a previously acquired fill slot.
    pub fn release(&self) {
        self.in_flight.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Guard that releases the stampede slot on drop.
#[derive(Debug)]
pub struct StampedePermit<'a> {
    budget: &'a StampedeBudget,
}

impl<'a> StampedePermit<'a> {
    /// Acquire or return `None` when the budget is full.
    pub fn try_acquire(budget: &'a StampedeBudget) -> Option<Self> {
        if budget.try_acquire() {
            Some(Self { budget })
        } else {
            None
        }
    }
}

impl Drop for StampedePermit<'_> {
    fn drop(&mut self) {
        self.budget.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iris_ir::CommitToken;

    #[test]
    fn authoritative_always_bypasses() {
        let intent = ConsistencyIntent::Authoritative;
        let wm = AppliedWatermarkState::new(10, 1_000);
        let ctx = CacheReadContext {
            intent: &intent,
            cache_wm: Some(&wm),
            session_fence: None,
            now_unix_ms: 2_000,
            cache_reachable: true,
        };
        assert_eq!(
            decide_cache_read(&ctx),
            CacheReadAction::BypassAuthority {
                reason: "authoritative_requires_authority"
            }
        );
    }

    #[test]
    fn ryw_misses_stale_cache() {
        let intent = ConsistencyIntent::ReadYourWrites;
        let wm = AppliedWatermarkState::new(5, 1_000);
        let fence = CommitToken::new(9);
        let ctx = CacheReadContext {
            intent: &intent,
            cache_wm: Some(&wm),
            session_fence: Some(&fence),
            now_unix_ms: 2_000,
            cache_reachable: true,
        };
        assert_eq!(
            decide_cache_read(&ctx),
            CacheReadAction::BypassAuthority {
                reason: "ryw_fence_not_covered"
            }
        );
    }

    #[test]
    fn ryw_hits_when_fence_covered() {
        let intent = ConsistencyIntent::ReadYourWrites;
        let wm = AppliedWatermarkState::new(9, 1_000);
        let fence = CommitToken::new(9);
        let ctx = CacheReadContext {
            intent: &intent,
            cache_wm: Some(&wm),
            session_fence: Some(&fence),
            now_unix_ms: 2_000,
            cache_reachable: true,
        };
        assert_eq!(
            decide_cache_read(&ctx),
            CacheReadAction::UseCache {
                freshness_proven: true
            }
        );
    }

    #[test]
    fn bounded_stale_rejects_unknown_watermark() {
        let intent = ConsistencyIntent::BoundedStale { max_lag_secs: 30 };
        let ctx = CacheReadContext {
            intent: &intent,
            cache_wm: None,
            session_fence: None,
            now_unix_ms: 10_000,
            cache_reachable: true,
        };
        assert_eq!(
            decide_cache_read(&ctx),
            CacheReadAction::BypassAuthority {
                reason: "bounded_stale_unknown_watermark"
            }
        );
    }

    #[test]
    fn bounded_stale_ok_within_lag() {
        let intent = ConsistencyIntent::BoundedStale { max_lag_secs: 5 };
        let wm = AppliedWatermarkState::new(3, 8_000);
        let ctx = CacheReadContext {
            intent: &intent,
            cache_wm: Some(&wm),
            session_fence: None,
            now_unix_ms: 12_000,
            cache_reachable: true,
        };
        assert_eq!(
            decide_cache_read(&ctx),
            CacheReadAction::UseCache {
                freshness_proven: true
            }
        );
    }

    #[test]
    fn stampede_budget_limits_concurrency() {
        let budget = StampedeBudget::new(1);
        let p1 = StampedePermit::try_acquire(&budget).expect("first");
        assert!(StampedePermit::try_acquire(&budget).is_none());
        drop(p1);
        assert!(StampedePermit::try_acquire(&budget).is_some());
    }
}
