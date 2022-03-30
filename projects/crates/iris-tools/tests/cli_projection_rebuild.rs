//! Phase 10-F CLI: projection rebuild (generation + alias).

use std::collections::BTreeMap;
use std::process::Command;

use iris::{
    ComponentRole, ConsistencyIntent, FallbackPolicy, IrisProject, LocalProjectionStore,
    ProjectionDocument, ProjectionGeneration, ProjectionPolicy, RouteRule, TOPOLOGY_FORMAT,
    TopologyComponent, TopologyContract,
};

fn iris_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_iris"))
}

#[test]
fn projection_rebuild_switches_alias() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("iris-rebuild-{stamp}"));
    std::fs::create_dir_all(dir.join("topologies")).unwrap();
    std::fs::write(dir.join("app.iris"), "table User { @@id: utf8, }\n").unwrap();

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
        "search".into(),
        TopologyComponent {
            role: ComponentRole::SearchProjection,
            adapter: "local".into(),
            adapter_version: None,
            datasource: None,
        },
    );
    let mut routes = BTreeMap::new();
    routes.insert(
        "search".into(),
        RouteRule {
            default_intent: ConsistencyIntent::Eventual,
            preferred_component: Some("search".into()),
            fallback: FallbackPolicy::FailClosed,
        },
    );
    let topo = TopologyContract {
        format: TOPOLOGY_FORMAT.into(),
        version: 1,
        id: "shop".into(),
        topology_version: 1,
        components,
        tables: BTreeMap::new(),
        routes,
        cache: Default::default(),
        outbox: Default::default(),
        object: Default::default(),
        projection: ProjectionPolicy {
            max_lag_secs: Some(30),
            keep_generations: Some(2),
            require_nonempty_rebuild: true,
            covered_fields: vec!["title".into()],
        },
    };
    let topo_path = dir.join("topologies").join("shop.von");
    std::fs::write(&topo_path, topo.to_von().unwrap()).unwrap();

    let mut project = IrisProject::with_schema("app.iris");
    project
        .topologies
        .insert("shop".into(), "topologies/shop.von".into());
    let config = dir.join("iris.von");
    project.save(&config).unwrap();

    let seed_path = dir.join("seed.von");
    #[derive(serde::Serialize)]
    struct Seed<'a> {
        format: &'a str,
        schema_fingerprint: &'a str,
        documents: Vec<ProjectionDocument>,
    }
    let seed = Seed {
        format: "iris.projection_seed",
        schema_fingerprint: "fp-test",
        documents: vec![ProjectionDocument {
            entity_id: "u1".into(),
            entity_version: 1,
            schema_fingerprint: "fp-test".into(),
            generation: ProjectionGeneration("pending".into()),
            text: Some("hello catalog".into()),
            vector: None,
            fields: Default::default(),
        }],
    };
    std::fs::write(&seed_path, von::to_string_indented(&seed).unwrap()).unwrap();

    let root = dir.join("projections");
    let out = dir.join("rebuild-status.von");
    let cfg = config.to_str().unwrap();
    let result = iris_bin()
        .args([
            "--config",
            cfg,
            "projection",
            "rebuild",
            "--topology",
            "shop",
            "--component",
            "search",
            "--root",
            root.to_str().unwrap(),
            "--seed",
            seed_path.to_str().unwrap(),
            "--yes",
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(result.status.success(), "{:?}", result);
    let body = std::fs::read_to_string(&out).unwrap();
    assert!(body.contains("search"));
    assert!(body.contains("active_generation") || body.contains("g"));

    let store = LocalProjectionStore::open(&root, ProjectionPolicy::default()).unwrap();
    let hits = store.search("search", "catalog", 5).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].entity_id, "u1");

    // Without --yes must fail.
    let result = iris_bin()
        .args([
            "--config",
            cfg,
            "projection",
            "rebuild",
            "--topology",
            "shop",
            "--component",
            "search",
            "--root",
            root.to_str().unwrap(),
            "--seed",
            seed_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!result.status.success());

    let _ = std::fs::remove_dir_all(dir);
}
