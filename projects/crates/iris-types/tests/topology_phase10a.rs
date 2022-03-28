//! Phase 10-A: TopologyContract offline validate + CompositePlan.

use std::collections::BTreeMap;

use iris_ir::{AccessKind, ConsistencyIntent};
use iris_types::{
    CachePolicy, ComponentRole, FallbackPolicy, ObjectPolicy, OutboxPolicy, ProjectionPolicy,
    RouteRule, TOPOLOGY_FORMAT, TableBinding, TopologyComponent, TopologyContract, verify_report,
};

fn sample_topology() -> TopologyContract {
    let mut components = BTreeMap::new();
    components.insert(
        "pg".into(),
        TopologyComponent {
            role: ComponentRole::Authority,
            adapter: "postgres".into(),
            adapter_version: Some("0.1.0".into()),
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
    components.insert(
        "outbox".into(),
        TopologyComponent {
            role: ComponentRole::Outbox,
            adapter: "postgres".into(),
            adapter_version: None,
            datasource: Some("main".into()),
        },
    );
    components.insert(
        "search".into(),
        TopologyComponent {
            role: ComponentRole::SearchProjection,
            adapter: "search".into(),
            adapter_version: None,
            datasource: None,
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
            default_intent: ConsistencyIntent::Authoritative,
            preferred_component: Some("redis".into()),
            fallback: FallbackPolicy::Authority,
        },
    );
    routes.insert(
        "search".into(),
        RouteRule {
            default_intent: ConsistencyIntent::ProjectionRequired {
                component: "search".into(),
            },
            preferred_component: Some("search".into()),
            fallback: FallbackPolicy::FailClosed,
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
            ttl_secs: Some(60),
            negative_ttl_secs: Some(5),
            stampede_budget: Some(32),
        },
        outbox: OutboxPolicy {
            ordering_domain: Some("entity".into()),
            dedupe_key: Some("operation_id+entity_version".into()),
            dead_letter: Some("poison".into()),
        },
        object: ObjectPolicy::default(),
        projection: ProjectionPolicy::default(),
    }
}

#[test]
fn topology_round_trips_von_and_rejects_dual_authority() {
    let topo = sample_topology();
    let text = topo.to_von().unwrap();
    assert!(text.contains(TOPOLOGY_FORMAT));
    assert!(text.contains("authority"));
    let parsed = TopologyContract::parse(&text).unwrap();
    assert_eq!(parsed.id, "commerce");
    assert_eq!(parsed.authority_id().unwrap(), "pg");

    let mut bad = sample_topology();
    bad.components.insert(
        "mysql".into(),
        TopologyComponent {
            role: ComponentRole::Authority,
            adapter: "mysql".into(),
            adapter_version: None,
            datasource: None,
        },
    );
    let err = bad.validate().unwrap_err();
    assert!(err.to_string().contains("exactly one Authority"));
}

#[test]
fn verify_report_mentions_roles() {
    let notes = verify_report(&sample_topology()).unwrap();
    assert!(notes.iter().any(|n| n.contains("authority: pg")));
    assert!(notes.iter().any(|n| n.starts_with("ok:")));
}

#[test]
fn plan_authoritative_identity_uses_authority_only() {
    let plan = sample_topology()
        .plan(
            AccessKind::IdentityRead,
            ConsistencyIntent::Authoritative,
            Some("User"),
        )
        .unwrap();
    assert!(!plan.rejected);
    assert_eq!(plan.authority_id, "pg");
    assert!(
        plan.steps
            .iter()
            .any(|s| matches!(s, iris_ir::CompositeStep::AuthorityStep { .. }))
    );
    assert!(!format!("{plan:?}").to_ascii_lowercase().contains("select "));
}

#[test]
fn plan_eventual_identity_includes_cache_and_fallback() {
    let plan = sample_topology()
        .plan(
            AccessKind::IdentityRead,
            ConsistencyIntent::Eventual,
            Some("User"),
        )
        .unwrap();
    assert!(!plan.rejected);
    assert!(
        plan.steps
            .iter()
            .any(|s| matches!(s, iris_ir::CompositeStep::DerivedReadStep { .. }))
    );
    assert!(
        plan.steps
            .iter()
            .any(|s| matches!(s, iris_ir::CompositeStep::FallbackStep { .. }))
    );
    assert!(plan.required_watermarks.contains_key("redis"));
}

#[test]
fn plan_ryw_identity_uses_fence_then_cache_fallback() {
    let plan = sample_topology()
        .plan(
            AccessKind::IdentityRead,
            ConsistencyIntent::ReadYourWrites,
            Some("User"),
        )
        .unwrap();
    assert!(!plan.rejected);
    assert!(plan.steps.iter().any(
        |s| matches!(s, iris_ir::CompositeStep::FenceStep { fence } if fence == "session_fence")
    ));
    assert!(
        plan.steps
            .iter()
            .any(|s| matches!(s, iris_ir::CompositeStep::DerivedReadStep { .. }))
    );
    assert!(plan.proof.notes.iter().any(|n| n.contains("session fence")));
}

#[test]
fn plan_write_includes_outbox_effect_not_dual_write() {
    let plan = sample_topology()
        .plan(AccessKind::Write, ConsistencyIntent::Authoritative, None)
        .unwrap();
    assert!(!plan.rejected);
    assert!(plan.steps.iter().any(|s| matches!(
        s,
        iris_ir::CompositeStep::AuthorityStep {
            append_outbox: true,
            ..
        }
    )));
    assert!(
        plan.steps
            .iter()
            .any(|s| matches!(s, iris_ir::CompositeStep::EffectStep { .. }))
    );
    assert!(
        plan.proof
            .notes
            .iter()
            .any(|n| n.contains("not sync dual-write"))
    );
}

#[test]
fn plan_write_with_object_store_is_pending_verify_finalize() {
    let mut topo = sample_topology();
    topo.components.insert(
        "objects".into(),
        TopologyComponent {
            role: ComponentRole::ObjectStore,
            adapter: "fs".into(),
            adapter_version: None,
            datasource: Some("objects".into()),
        },
    );
    topo.object = ObjectPolicy {
        hash_alg: Some("blake3".into()),
        orphan_ttl_secs: Some(3600),
        pending_ttl_secs: None,
    };
    let plan = topo
        .plan(AccessKind::Write, ConsistencyIntent::Authoritative, None)
        .unwrap();
    assert!(!plan.rejected);
    let actions: Vec<_> = plan
        .steps
        .iter()
        .filter_map(|s| match s {
            iris_ir::CompositeStep::ObjectStep { action, .. } => Some(action.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(actions, vec!["pending", "write", "verify", "finalize"]);
    assert!(
        plan.budget_notes
            .iter()
            .any(|n| n.contains("object_orphan_ttl_secs"))
    );

    let bytes = topo
        .plan(
            AccessKind::BytesRange,
            ConsistencyIntent::Authoritative,
            None,
        )
        .unwrap();
    assert!(!bytes.rejected);
    assert!(bytes.steps.iter().any(
        |s| matches!(s, iris_ir::CompositeStep::ObjectStep { action, .. } if action == "range_read")
    ));
}

#[test]
fn plan_search_without_projection_rejects_approximation() {
    let mut topo = sample_topology();
    topo.components.remove("search");
    topo.routes.remove("search");
    let plan = topo
        .plan(AccessKind::Search, ConsistencyIntent::Eventual, None)
        .unwrap();
    assert!(plan.rejected);
    assert!(
        plan.rejection
            .as_deref()
            .unwrap_or("")
            .contains("refuse approximate")
    );
}

#[test]
fn plan_search_hydrates_through_authority() {
    let plan = sample_topology()
        .plan(AccessKind::Search, ConsistencyIntent::Eventual, None)
        .unwrap();
    assert!(!plan.rejected);
    assert!(
        plan.steps
            .iter()
            .any(|s| matches!(s, iris_ir::CompositeStep::HydrateStep { .. }))
    );
    assert!(
        plan.steps
            .iter()
            .any(|s| matches!(s, iris_ir::CompositeStep::CompletenessCheck { .. }))
    );
    assert!(
        plan.budget_notes
            .iter()
            .any(|n| n.contains("candidates_only"))
    );
}
