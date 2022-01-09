//! Composite Backend plan shapes (Phase 10-A skeleton).
//!
//! Backend-neutral: no middleware private commands, no SQL, no dual-authority
//! transactions.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Application-facing consistency intent (backend-neutral).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConsistencyIntent {
    /// Only authority committed results.
    Authoritative,
    /// Session writes must be visible (fence / watermark).
    ReadYourWrites,
    /// Derived read allowed when watermark lag ≤ bound (seconds).
    BoundedStale {
        /// Maximum acceptable lag in seconds.
        max_lag_secs: u64,
    },
    /// Any healthy derived projection; no freshness proof required.
    Eventual,
    /// Operation requires a named projection role; fail if unavailable.
    ProjectionRequired {
        /// Component id that must serve the operation.
        component: String,
    },
}

/// High-level access kind derived from VOS IR (Phase 10-A offline sketch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessKind {
    /// Primary-key identity read.
    IdentityRead,
    /// Filtered / relational query.
    FilteredQuery,
    /// Full-text / search projection.
    Search,
    /// Vector nearest / ANN.
    VectorNearest,
    /// Authority write (mutate + outbox).
    Write,
    /// Object bytes range read.
    BytesRange,
    /// Post-commit effect (outbox consumer).
    Effect,
}

/// Why a route was chosen (explain / conformance; no secrets).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteProof {
    /// Human-readable proof notes (capability ∩ consistency ∩ mapping).
    pub notes: Vec<String>,
    /// Whether freshness can be proven from watermarks for this route.
    pub freshness_proven: bool,
}

impl RouteProof {
    /// Empty proof (rejected / not yet filled).
    pub fn empty() -> Self {
        Self {
            notes: Vec::new(),
            freshness_proven: false,
        }
    }

    /// Single-note proof.
    pub fn note(text: impl Into<String>, freshness_proven: bool) -> Self {
        Self {
            notes: vec![text.into()],
            freshness_proven,
        }
    }
}

/// One step in a [`CompositePlan`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CompositeStep {
    /// Authority read/write (and optional outbox append for writes).
    AuthorityStep {
        /// Component id with role=authority.
        component: String,
        /// When true, plan includes outbox append in the same authority txn.
        append_outbox: bool,
    },
    /// Derived reader (cache / search / vector / replica).
    DerivedReadStep {
        /// Component id.
        component: String,
        /// Required applied watermark relative to authority commit token when known.
        required_watermark: Option<String>,
    },
    /// Batch load identities from authority and re-validate.
    HydrateStep {
        /// Authority component id.
        component: String,
    },
    /// Object store pending/write/finalize/range-read.
    ObjectStep {
        /// Object-store component id.
        component: String,
        /// Object lifecycle action label (no private protocol text).
        action: String,
    },
    /// Wait/check commit token vs applied watermark.
    FenceStep {
        /// Fence label (session fence or commit token id).
        fence: String,
    },
    /// Pre-approved equivalent fallback route.
    FallbackStep {
        /// Why fallback triggers.
        reason: String,
        /// Replacement steps (must themselves be proven).
        steps: Vec<CompositeStep>,
    },
    /// Post-commit effect driven by outbox.
    EffectStep {
        /// Outbox / event-log component id.
        component: String,
    },
    /// Completeness / ghost / stale-candidate filtering.
    CompletenessCheck {
        /// Policy note.
        policy: String,
    },
}

/// Versioned composite execution plan (Phase 10-A: offline planning only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositePlan {
    /// Discriminator.
    pub format: String,
    /// Plan document version.
    pub version: i64,
    /// Topology id.
    pub topology_id: String,
    /// Topology contract version used to build this plan.
    pub topology_version: i64,
    /// Authority component id.
    pub authority_id: String,
    /// Access kind.
    pub access: AccessKind,
    /// Consistency intent.
    pub consistency: ConsistencyIntent,
    /// Ordered steps.
    pub steps: Vec<CompositeStep>,
    /// Route proof.
    pub proof: RouteProof,
    /// Required watermarks (component -> token label).
    #[serde(default)]
    pub required_watermarks: BTreeMap<String, String>,
    /// Compensation / concurrency budget notes (opaque labels for 10-A).
    #[serde(default)]
    pub budget_notes: Vec<String>,
    /// When true, plan was rejected before execution.
    #[serde(default)]
    pub rejected: bool,
    /// Rejection reason when [`Self::rejected`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejection: Option<String>,
}

/// Stable format discriminator for composite plans.
pub const COMPOSITE_PLAN_FORMAT: &str = "iris.composite_plan";

impl CompositePlan {
    /// Build a rejected plan (no middleware commands).
    pub fn rejected(
        topology_id: impl Into<String>,
        topology_version: i64,
        authority_id: impl Into<String>,
        access: AccessKind,
        consistency: ConsistencyIntent,
        reason: impl Into<String>,
    ) -> Self {
        let reason = reason.into();
        Self {
            format: COMPOSITE_PLAN_FORMAT.into(),
            version: 1,
            topology_id: topology_id.into(),
            topology_version,
            authority_id: authority_id.into(),
            access,
            consistency,
            steps: Vec::new(),
            proof: RouteProof::note(reason.clone(), false),
            required_watermarks: BTreeMap::new(),
            budget_notes: Vec::new(),
            rejected: true,
            rejection: Some(reason),
        }
    }
}
