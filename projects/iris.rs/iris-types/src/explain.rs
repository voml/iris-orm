//! Composite + optional physical explain reports (Phase 10-D).
//!
//! Public explain output must never include credentials, parameter values, or
//! foreign-store private command text (SQL / Redis wire commands).

use iris_ir::{
    AccessKind, COMPOSITE_PLAN_FORMAT, CompositePlan, CompositeStep, ConsistencyIntent,
    PhysicalPlan, RealizationClass,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::topology::{ComponentRole, TopologyContract, TopologyError};

/// Stable format discriminator for explain reports.
pub const EXPLAIN_FORMAT: &str = "iris.explain";

/// One composite step summarized for operators (role ids, not private commands).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplainStep {
    /// Step kind label (`authority`, `derived_read`, `fallback`, ...).
    pub kind: String,
    /// Topology component id when applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,
    /// Declared role for that component.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Non-secret detail (fence label, append_outbox, fallback reason, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Physical-plan sketch section (optional; from VOS via Planner).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhysicalExplain {
    /// Whether planning failed / rejected.
    pub rejected: bool,
    /// Backend id used for capability classification (sketch label).
    pub backend_sketch: String,
    /// Per-node `Op:Realization` lines (no literals / bind values).
    pub nodes: Vec<String>,
    /// Optional planner note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Safety scan results for the emitted explain artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ExplainSafety {
    /// True if credential-like tokens were detected (report must not be emitted).
    pub credentials_suspected: bool,
    /// True if SQL-shaped text was detected.
    pub sql_shaped_suspected: bool,
    /// True if private middleware command text was detected.
    pub private_command_suspected: bool,
}

impl ExplainSafety {
    /// True when any suspicion bit is set.
    pub fn is_dirty(&self) -> bool {
        self.credentials_suspected || self.sql_shaped_suspected || self.private_command_suspected
    }
}

/// Operator-facing explain report (VON-serializable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplainReport {
    /// Discriminator.
    pub format: String,
    /// Report document version.
    pub version: i64,
    /// Topology id.
    pub topology_id: String,
    /// Topology contract version.
    pub topology_version: i64,
    /// Access kind explained.
    pub access: AccessKind,
    /// Consistency intent.
    pub consistency: ConsistencyIntent,
    /// Optional table binding context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
    /// Sole authority component id.
    pub authority_id: String,
    /// Whether the composite plan was rejected.
    pub rejected: bool,
    /// Rejection reason when rejected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejection: Option<String>,
    /// Ordered composite steps (role-level).
    pub steps: Vec<ExplainStep>,
    /// Route proof notes.
    pub proof_notes: Vec<String>,
    /// Whether freshness is proven for this route sketch.
    pub freshness_proven: bool,
    /// Required watermarks (component -> token label).
    #[serde(default)]
    pub required_watermarks: BTreeMap<String, String>,
    /// Fallback reasons extracted from the composite plan.
    #[serde(default)]
    pub fallbacks: Vec<String>,
    /// Budget notes (e.g. stampede_budget).
    #[serde(default)]
    pub budget_notes: Vec<String>,
    /// Embedded composite plan format marker (for tooling).
    pub composite_plan_format: String,
    /// Optional physical sketch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physical: Option<PhysicalExplain>,
    /// Safety scan (should be all-false for a valid emit).
    pub safety: ExplainSafety,
}

/// Build an explain report from a topology plan (+ optional physical sketch).
pub fn explain_topology(
    topo: &TopologyContract,
    access: AccessKind,
    consistency: ConsistencyIntent,
    table: Option<&str>,
    physical: Option<PhysicalExplain>,
) -> Result<ExplainReport, TopologyError> {
    let plan = topo.plan(access, consistency.clone(), table)?;
    Ok(explain_from_plan(topo, table, plan, physical))
}

/// Build an explain report from an already-computed [`CompositePlan`].
pub fn explain_from_plan(
    topo: &TopologyContract,
    table: Option<&str>,
    plan: CompositePlan,
    physical: Option<PhysicalExplain>,
) -> ExplainReport {
    let mut fallbacks = Vec::new();
    let steps = plan
        .steps
        .iter()
        .map(|s| summarize_step(topo, s, &mut fallbacks))
        .collect();

    let mut report = ExplainReport {
        format: EXPLAIN_FORMAT.into(),
        version: 1,
        topology_id: plan.topology_id.clone(),
        topology_version: plan.topology_version,
        access: plan.access,
        consistency: plan.consistency.clone(),
        table: table.map(str::to_string),
        authority_id: plan.authority_id.clone(),
        rejected: plan.rejected,
        rejection: plan.rejection.clone(),
        steps,
        proof_notes: plan.proof.notes.clone(),
        freshness_proven: plan.proof.freshness_proven,
        required_watermarks: plan.required_watermarks.clone(),
        fallbacks,
        budget_notes: plan.budget_notes.clone(),
        composite_plan_format: COMPOSITE_PLAN_FORMAT.into(),
        physical,
        safety: ExplainSafety::default(),
    };
    report.safety = scan_explain_text(&format!("{report:?}"));
    report
}

/// Summarize a physical plan without embedding bind values or store commands.
pub fn physical_explain_from_plan(plan: &PhysicalPlan, backend_sketch: &str) -> PhysicalExplain {
    let mut nodes = Vec::new();
    let mut note = None;
    let mut rejected = plan.is_rejected();
    for n in &plan.nodes {
        let op_label = match &n.op {
            iris_ir::PhysicalOp::Scan { .. } => "Scan",
            iris_ir::PhysicalOp::Filter { .. } => "Filter",
            iris_ir::PhysicalOp::Sort { .. } => "Sort",
            iris_ir::PhysicalOp::Skip { .. } => "Skip",
            iris_ir::PhysicalOp::Take { .. } => "Take",
            iris_ir::PhysicalOp::Project { .. } => "Project",
            iris_ir::PhysicalOp::Collect => "Collect",
        };
        let real = match n.realization {
            RealizationClass::Native => "Native",
            RealizationClass::Equivalent => "Equivalent",
            RealizationClass::Compensated => "Compensated",
            RealizationClass::Rejected => {
                rejected = true;
                "Rejected"
            }
        };
        nodes.push(format!("{op_label}:{real}"));
        if let Some(ref nnote) = n.note {
            note = Some(redact_operator_note(nnote));
        }
    }
    if let Some(rej) = plan.rejection_note() {
        note = Some(redact_operator_note(rej));
        rejected = true;
    }
    PhysicalExplain {
        rejected,
        backend_sketch: backend_sketch.into(),
        nodes,
        note,
    }
}

fn summarize_step(
    topo: &TopologyContract,
    step: &CompositeStep,
    fallbacks: &mut Vec<String>,
) -> ExplainStep {
    match step {
        CompositeStep::AuthorityStep {
            component,
            append_outbox,
        } => ExplainStep {
            kind: "authority".into(),
            component: Some(component.clone()),
            role: role_of(topo, component),
            detail: Some(format!("append_outbox={append_outbox}")),
        },
        CompositeStep::DerivedReadStep {
            component,
            required_watermark,
        } => ExplainStep {
            kind: "derived_read".into(),
            component: Some(component.clone()),
            role: role_of(topo, component),
            detail: required_watermark
                .as_ref()
                .map(|w| format!("required_watermark={w}")),
        },
        CompositeStep::HydrateStep { component } => ExplainStep {
            kind: "hydrate".into(),
            component: Some(component.clone()),
            role: role_of(topo, component),
            detail: None,
        },
        CompositeStep::ObjectStep { component, action } => ExplainStep {
            kind: "object".into(),
            component: Some(component.clone()),
            role: role_of(topo, component),
            detail: Some(format!("action={action}")),
        },
        CompositeStep::FenceStep { fence } => ExplainStep {
            kind: "fence".into(),
            component: None,
            role: None,
            detail: Some(format!("fence={fence}")),
        },
        CompositeStep::FallbackStep { reason, steps } => {
            fallbacks.push(reason.clone());
            ExplainStep {
                kind: "fallback".into(),
                component: None,
                role: None,
                detail: Some(format!("reason={reason}; nested_steps={}", steps.len())),
            }
        }
        CompositeStep::EffectStep { component } => ExplainStep {
            kind: "effect".into(),
            component: Some(component.clone()),
            role: role_of(topo, component),
            detail: None,
        },
        CompositeStep::CompletenessCheck { policy } => ExplainStep {
            kind: "completeness_check".into(),
            component: None,
            role: None,
            detail: Some(format!("policy={policy}")),
        },
    }
}

fn role_of(topo: &TopologyContract, component: &str) -> Option<String> {
    topo.components.get(component).map(|c| role_label(c.role))
}

fn role_label(role: ComponentRole) -> String {
    match role {
        ComponentRole::Authority => "authority",
        ComponentRole::Cache => "cache",
        ComponentRole::SearchProjection => "search_projection",
        ComponentRole::VectorProjection => "vector_projection",
        ComponentRole::ObjectStore => "object_store",
        ComponentRole::Outbox => "outbox",
        ComponentRole::Queue => "queue",
        ComponentRole::Replica => "replica",
        ComponentRole::Lock => "lock",
    }
    .into()
}

fn redact_operator_note(note: &str) -> String {
    // Keep short; strip anything that looks like a bind/literal dump.
    let muted = note
        .replace(['\'', '"'], "")
        .chars()
        .take(160)
        .collect::<String>();
    muted
}

/// Scan text for disallowed explain content.
pub fn scan_explain_text(text: &str) -> ExplainSafety {
    let lower = text.to_ascii_lowercase();
    let credentials_suspected = lower.contains("password=")
        || lower.contains("passwd=")
        || lower.contains("secret=")
        || lower.contains("://")
            && (lower.contains("@") && lower.contains("redis://")
                || lower.contains("postgres://")
                || lower.contains("mysql://"));
    // Broader credential patterns without requiring URL schemes in Debug output.
    let credentials_suspected = credentials_suspected
        || lower.contains("password:")
        || lower.contains("api_key")
        || lower.contains("authorization:");

    let sql_shaped_suspected = lower.contains("select ")
        || lower.contains("create table")
        || lower.contains("insert into")
        || lower.contains("alter table")
        || lower.contains("drop table");

    let private_command_suspected = lower.contains(" cmd ")
        || lower.contains("hgetall")
        || lower.contains("zrange")
        || lower.contains(" scan ")
        || lower.contains("explain analyze");

    ExplainSafety {
        credentials_suspected,
        sql_shaped_suspected,
        private_command_suspected,
    }
}

/// Refuse to emit if the serialized explain body looks unsafe.
pub fn assert_explain_safe(text: &str) -> Result<(), String> {
    let safety = scan_explain_text(text);
    if safety.is_dirty() {
        return Err(format!(
            "refusing to emit explain: credentials={} sql_shaped={} private_command={}",
            safety.credentials_suspected,
            safety.sql_shaped_suspected,
            safety.private_command_suspected
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::topology::{
        CachePolicy, FallbackPolicy, OutboxPolicy, RouteRule, TOPOLOGY_FORMAT, TableBinding,
        TopologyComponent,
    };
    use std::collections::BTreeMap;

    fn sample() -> TopologyContract {
        let mut components = BTreeMap::new();
        components.insert(
            "pg".into(),
            TopologyComponent {
                role: ComponentRole::Authority,
                adapter: "postgres".into(),
                adapter_version: None,
                datasource: Some("main".into()),
            },
        );
        components.insert(
            "redis".into(),
            TopologyComponent {
                role: ComponentRole::Cache,
                adapter: "redis".into(),
                adapter_version: None,
                datasource: Some("cache".into()),
            },
        );
        let mut tables = BTreeMap::new();
        tables.insert(
            "User".into(),
            TableBinding {
                authority: "pg".into(),
            },
        );
        let mut routes = BTreeMap::new();
        routes.insert(
            "identity_read".into(),
            RouteRule {
                default_intent: ConsistencyIntent::Eventual,
                preferred_component: Some("redis".into()),
                fallback: FallbackPolicy::Authority,
            },
        );
        TopologyContract {
            format: TOPOLOGY_FORMAT.into(),
            version: 1,
            id: "commerce".into(),
            topology_version: 1,
            components,
            tables,
            routes,
            cache: CachePolicy {
                ttl_secs: Some(30),
                negative_ttl_secs: None,
                stampede_budget: Some(4),
            },
            outbox: OutboxPolicy::default(),
            object: crate::topology::ObjectPolicy::default(),
            projection: crate::topology::ProjectionPolicy::default(),
        }
    }

    #[test]
    fn explain_eventual_includes_cache_fallback_and_budget() {
        let report = explain_topology(
            &sample(),
            AccessKind::IdentityRead,
            ConsistencyIntent::Eventual,
            Some("User"),
            None,
        )
        .unwrap();
        assert_eq!(report.format, EXPLAIN_FORMAT);
        assert!(!report.rejected);
        assert!(report.steps.iter().any(|s| s.kind == "derived_read"));
        assert!(!report.fallbacks.is_empty());
        assert!(report.required_watermarks.contains_key("redis"));
        assert!(
            report
                .budget_notes
                .iter()
                .any(|b| b.contains("stampede_budget"))
        );
        assert!(!report.safety.is_dirty());
    }

    #[test]
    fn scan_rejects_sql_shaped() {
        let s = scan_explain_text("oops SELECT * from users");
        assert!(s.sql_shaped_suspected);
        assert!(assert_explain_safe("oops SELECT * from users").is_err());
    }
}
