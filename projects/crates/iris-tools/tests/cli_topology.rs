//! Phase 10-A CLI: topology verify / plan (offline).

use std::collections::BTreeMap;
use std::process::Command;

use iris::{
    CachePolicy, ComponentRole, ConsistencyIntent, FallbackPolicy, IrisProject, OutboxPolicy,
    RouteRule, TOPOLOGY_FORMAT, TableBinding, TopologyComponent, TopologyContract,
};

fn iris_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_iris"))
}

fn sample_topology() -> TopologyContract {
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
            stampede_budget: Some(8),
        },
        outbox: OutboxPolicy::default(),
        object: iris::ObjectPolicy::default(),
        projection: iris::ProjectionPolicy::default(),
    }
}

#[test]
fn topology_verify_and_plan_offline() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("iris-topo-{stamp}"));
    std::fs::create_dir_all(dir.join("topologies")).unwrap();
    std::fs::write(dir.join("app.iris"), "table User { @@id: utf8, }\n").unwrap();

    let topo_path = dir.join("topologies").join("commerce.von");
    sample_topology().save_via_von(&topo_path);

    let mut project = IrisProject::with_schema("app.iris");
    project
        .topologies
        .insert("commerce".into(), "topologies/commerce.von".into());
    let config = dir.join("iris.von");
    project.save(&config).unwrap();
    let cfg = config.to_str().unwrap();

    let out = iris_bin()
        .args([
            "--config",
            cfg,
            "topology",
            "verify",
            "--topology",
            "commerce",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("authority: pg"));
    assert!(!stdout.to_ascii_lowercase().contains("select "));

    let plan_out = dir.join("plans").join("commerce-identity.von");
    let out = iris_bin()
        .args([
            "--config",
            cfg,
            "topology",
            "plan",
            "--topology",
            "commerce",
            "--op",
            "identity_read",
            "--intent",
            "eventual",
            "--table",
            "User",
            "--out",
            plan_out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let plan_text = std::fs::read_to_string(&plan_out).unwrap();
    assert!(plan_text.contains("iris.composite_plan"));
    assert!(plan_text.contains("derived_read_step") || plan_text.contains("DerivedRead"));
    assert!(!plan_text.to_ascii_lowercase().contains("create table"));

    let out = iris_bin()
        .args([
            "--config",
            cfg,
            "topology",
            "activate",
            "--topology",
            "commerce",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("activate ok") || stdout.contains("wrote"));
    let active = dir.join("topologies").join("commerce.active.von");
    assert!(
        active.exists(),
        "expected activation record at {}",
        active.display()
    );
    let body = std::fs::read_to_string(&active).unwrap();
    assert!(body.contains("iris.topology_activation"));
    assert!(!body.to_ascii_lowercase().contains("select "));

    let _ = std::fs::remove_dir_all(dir);
}

trait SaveVon {
    fn save_via_von(&self, path: &std::path::Path);
}

impl SaveVon for TopologyContract {
    fn save_via_von(&self, path: &std::path::Path) {
        let text = self.to_von().unwrap();
        std::fs::write(path, text).unwrap();
    }
}
