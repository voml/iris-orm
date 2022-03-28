//! Phase 10-D CLI: iris explain + projection status (Cache).

use std::collections::BTreeMap;
use std::process::Command;

use iris::{
    CachePolicy, ComponentRole, ConsistencyIntent, DatasourceConfig, DatasourceKind,
    FallbackPolicy, IrisProject, ObjectPolicy, OutboxPolicy, ProjectionPolicy, RouteRule,
    TOPOLOGY_FORMAT, TableBinding, TopologyComponent, TopologyContract, TruthMode,
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
        object: ObjectPolicy::default(),
        projection: ProjectionPolicy::default(),
    }
}

#[test]
fn explain_and_projection_status_offline() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("iris-explain-{stamp}"));
    std::fs::create_dir_all(dir.join("topologies")).unwrap();
    std::fs::write(dir.join("app.iris"), "table User { @@id: utf8, }\n").unwrap();

    let topo_path = dir.join("topologies").join("commerce.von");
    let text = sample_topology().to_von().unwrap();
    std::fs::write(&topo_path, text).unwrap();

    let mut project = IrisProject::with_schema("app.iris");
    project
        .topologies
        .insert("commerce".into(), "topologies/commerce.von".into());
    project.datasources.insert(
        "main".into(),
        DatasourceConfig {
            kind: DatasourceKind::Postgres,
            mode: TruthMode::ManagedPush,
            path: None,
            url: Some("$IRIS_TEST_POSTGRES_URL".into()),
        },
    );
    project.datasources.insert(
        "cache".into(),
        DatasourceConfig {
            kind: DatasourceKind::Redis,
            mode: TruthMode::AdoptExisting,
            path: None,
            url: Some("$IRIS_TEST_REDIS_URL".into()),
        },
    );
    let config = dir.join("iris.von");
    project.save(&config).unwrap();
    let cfg = config.to_str().unwrap();

    let explain_out = dir.join("plans").join("explain.von");
    let out = iris_bin()
        .args([
            "--config",
            cfg,
            "explain",
            "--topology",
            "commerce",
            "--op",
            "identity_read",
            "--intent",
            "eventual",
            "--table",
            "User",
            "--out",
            explain_out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let body = std::fs::read_to_string(&explain_out).unwrap();
    assert!(body.contains("iris.explain"));
    assert!(body.contains("derived_read") || body.contains("DerivedRead"));
    assert!(body.contains("fallback") || body.contains("stampede"));
    let lower = body.to_ascii_lowercase();
    assert!(!lower.contains("select "));
    assert!(!lower.contains("create table"));
    assert!(!lower.contains("password="));
    assert!(!lower.contains("hgetall"));

    let status_out = dir.join("plans").join("proj-status.von");
    let out = iris_bin()
        .args([
            "--config",
            cfg,
            "projection",
            "status",
            "--topology",
            "commerce",
            "--component",
            "redis",
            "--out",
            status_out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let body = std::fs::read_to_string(&status_out).unwrap();
    assert!(body.contains("iris.projection_status"));
    assert!(body.contains("redis"));
    assert!(body.contains("cache"));
    assert!(!body.to_ascii_lowercase().contains("select "));

    let out = iris_bin()
        .args([
            "--config",
            cfg,
            "projection",
            "verify",
            "--topology",
            "commerce",
            "--component",
            "redis",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("projection verify") || stdout.contains("wrote"));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn help_lists_explain_and_projection() {
    let out = iris_bin().arg("--help").output().unwrap();
    assert!(out.status.success());
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(help.contains("explain"));
    assert!(help.contains("projection"));
}
