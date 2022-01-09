//! Phase 10-G / §15.6 Composite Backend conformance (offline).

use std::collections::BTreeMap;

use iris_ir::{
    AccessKind, CommitToken, ConsistencyIntent, ProjectionDocument, ProjectionGeneration,
};
use iris_types::{
    AppliedWatermarkState, AuthorityEntity, CachePolicy, CacheReadAction, CacheReadContext,
    ComponentRole, FallbackPolicy, LocalProjectionStore, MapAuthorityLookup, ObjectPolicy,
    OutboxPolicy, ProjectionPolicy, RouteRule, StampedeBudget, StampedePermit, TOPOLOGY_FORMAT,
    TableBinding, TopologyComponent, TopologyContract, activate_topology, decide_cache_read,
    hydrate_candidates, verify_projection,
};

fn commerce() -> TopologyContract {
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
        "search".into(),
        TopologyComponent {
            role: ComponentRole::SearchProjection,
            adapter: "local".into(),
            adapter_version: None,
            datasource: None,
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
            default_intent: ConsistencyIntent::Eventual,
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
            stampede_budget: Some(4),
            ..CachePolicy::default()
        },
        outbox: OutboxPolicy::default(),
        object: ObjectPolicy::default(),
        projection: ProjectionPolicy {
            require_nonempty_rebuild: true,
            covered_fields: vec!["title".into()],
            ..ProjectionPolicy::default()
        },
    }
}

fn step_kinds(plan: &iris_ir::CompositePlan) -> Vec<&'static str> {
    plan.steps
        .iter()
        .map(|s| match s {
            iris_ir::CompositeStep::AuthorityStep { .. } => "authority",
            iris_ir::CompositeStep::DerivedReadStep { .. } => "derived_read",
            iris_ir::CompositeStep::HydrateStep { .. } => "hydrate",
            iris_ir::CompositeStep::ObjectStep { .. } => "object",
            iris_ir::CompositeStep::FenceStep { .. } => "fence",
            iris_ir::CompositeStep::FallbackStep { .. } => "fallback",
            iris_ir::CompositeStep::EffectStep { .. } => "effect",
            iris_ir::CompositeStep::CompletenessCheck { .. } => "completeness_check",
        })
        .collect()
}

#[test]
fn s15_6_write_never_promotes_cache_as_write_truth() {
    let plan = commerce()
        .plan(AccessKind::Write, ConsistencyIntent::Authoritative, None)
        .unwrap();
    assert!(!plan.rejected);
    assert!(step_kinds(&plan).contains(&"authority"));
    assert!(!step_kinds(&plan).contains(&"derived_read"));
}

#[test]
fn s15_6_ryw_and_bounded_stale_freshness() {
    let intent = ConsistencyIntent::ReadYourWrites;
    let wm = AppliedWatermarkState::new(1, 1000);
    let fence = CommitToken::new(9);
    let ctx = CacheReadContext {
        intent: &intent,
        cache_wm: Some(&wm),
        session_fence: Some(&fence),
        now_unix_ms: 2000,
        cache_reachable: true,
    };
    assert_eq!(
        decide_cache_read(&ctx),
        CacheReadAction::BypassAuthority {
            reason: "ryw_fence_not_covered"
        }
    );

    let intent = ConsistencyIntent::BoundedStale { max_lag_secs: 30 };
    let ctx = CacheReadContext {
        intent: &intent,
        cache_wm: None,
        session_fence: None,
        now_unix_ms: 2000,
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
fn s15_6_search_fail_closed_without_projection_and_hydrates_with() {
    let mut topo = commerce();
    topo.components.remove("search");
    topo.routes.remove("search");
    let rejected = topo
        .plan(AccessKind::Search, ConsistencyIntent::Eventual, None)
        .unwrap();
    assert!(rejected.rejected);
    assert!(
        rejected
            .rejection
            .as_deref()
            .unwrap_or("")
            .contains("refuse approximate")
    );

    let ok = commerce()
        .plan(AccessKind::Search, ConsistencyIntent::Eventual, None)
        .unwrap();
    let kinds = step_kinds(&ok);
    assert!(kinds.contains(&"derived_read"));
    assert!(kinds.contains(&"hydrate"));
    assert!(kinds.contains(&"completeness_check"));
}

#[test]
fn s15_6_hydrate_drops_ghosts_and_stampede_budget() {
    let mut auth = MapAuthorityLookup::default();
    auth.rows.insert(
        "a".into(),
        AuthorityEntity {
            entity_id: "a".into(),
            entity_version: 2,
            deleted: false,
            payload: None,
        },
    );
    let cands = vec![iris_ir::ProjectionCandidate {
        entity_id: "a".into(),
        score: 1.0,
        entity_version: 1,
        generation: ProjectionGeneration("g".into()),
        schema_fingerprint: "fp".into(),
    }];
    let r = hydrate_candidates(&cands, &auth).unwrap();
    assert!(r.entities.is_empty());
    assert_eq!(r.completeness.dropped_version, 1);

    let budget = StampedeBudget::new(1);
    let _p = StampedePermit::try_acquire(&budget).unwrap();
    assert!(StampedePermit::try_acquire(&budget).is_none());
}

#[test]
fn s15_6_rebuild_isolation_activate_handshake_and_projection_verify() {
    let topo = commerce();
    let root = std::env::temp_dir().join(format!(
        "iris-c15-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = LocalProjectionStore::open(&root, topo.projection.clone()).unwrap();
    let h = store.begin_rebuild("search", "fp", 10).unwrap();
    store
        .upsert_building(
            &h,
            ProjectionDocument {
                entity_id: "u1".into(),
                entity_version: 1,
                schema_fingerprint: "fp".into(),
                generation: ProjectionGeneration("x".into()),
                text: Some("alpha".into()),
                vector: None,
                fields: Default::default(),
            },
            11,
        )
        .unwrap();
    // Building must not be served until activate.
    assert!(store.search("search", "alpha", 5).is_err());
    store.activate(&h, 12).unwrap();
    assert_eq!(store.search("search", "alpha", 5).unwrap().len(), 1);

    let v = verify_projection(&topo, "search", Some(&store)).unwrap();
    assert!(v.ok, "{v:?}");

    let state = root.join("topology-state");
    let a1 = activate_topology(&topo, &state, 100, false).unwrap();
    assert!(a1.ok);
    let mut v2 = topo.clone();
    v2.topology_version = 2;
    let a2 = activate_topology(&v2, &state, 200, false).unwrap();
    assert!(a2.ok);
    let act = a2.activation.unwrap();
    assert_eq!(act.handshake.min_reader_version, 1);
    assert_eq!(act.handshake.writer_version, 2);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn s15_6_dual_authority_rejected() {
    let mut topo = commerce();
    topo.components.insert(
        "mysql".into(),
        TopologyComponent {
            role: ComponentRole::Authority,
            adapter: "mysql".into(),
            adapter_version: None,
            datasource: None,
        },
    );
    assert!(topo.validate().is_err());
}
