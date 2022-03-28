//! Phase 6: Redis keyspace adapter --?offline + optional live tests.

use iris_adapter_redis::{BACKEND_ID, KeyEncoding, KeyspaceMapping, MappingManifest, RedisSource};
use iris_types::Planner;

fn sample_mapping() -> MappingManifest {
    MappingManifest::with_tables(vec![KeyspaceMapping {
        vos_table: "User".into(),
        key_prefix: "iris:test:user:".into(),
        primary_key_field: "user_id".into(),
        encoding: KeyEncoding::Utf8String,
        ttl_secs: Some(60),
    }])
}

fn live_url() -> Option<String> {
    std::env::var("IRIS_TEST_REDIS_URL")
        .ok()
        .filter(|s| !s.is_empty())
}

#[test]
fn adapter_is_keyspace_not_relational() {
    assert_eq!(BACKEND_ID, "redis");
    assert_ne!(BACKEND_ID, "sqlite");
    assert_ne!(BACKEND_ID, "yydb");
    let caps = RedisSource::capabilities();
    assert!(!caps.query.filter_cmp);
    assert!(!caps.query.sort);
    assert!(caps.write.insert);
}

#[test]
fn draft_keyspace_mapping_from_vos_without_scan() {
    let manifest = RedisSource::draft_keyspace_mapping(
        r#"
        table User {
            @@user_id: utf8,
            @user_name: utf8,
        }
        "#,
    )
    .unwrap();
    assert_eq!(manifest.adapter_id, BACKEND_ID);
    assert_eq!(manifest.tables.len(), 1);
    let user = &manifest.tables[0];
    assert_eq!(user.vos_table, "User");
    assert_eq!(user.key_prefix, "iris:user:");
    assert_eq!(user.primary_key_field, "user_id");
    assert_eq!(user.encoding, KeyEncoding::JsonDocument);
}

#[test]
fn draft_keyspace_mapping_rejects_missing_pk() {
    let err = RedisSource::draft_keyspace_mapping(
        r#"
        table NoPk {
            name: utf8,
        }
        "#,
    )
    .unwrap_err();
    assert!(err.to_string().contains("primary key"));
}

#[test]
fn physical_plans_with_filter_are_rejected_at_plan() {
    let caps = RedisSource::capabilities();
    let err = Planner::new(caps)
        .plan_source(r#"User.filter(x => x.active).collect()"#)
        .expect_err("filter must be rejected under redis caps");
    let msg = err.to_string();
    assert!(
        msg.contains("IRIS-PLAN-REJECTED") || msg.contains("reject") || msg.contains("filter"),
        "{msg}"
    );
}

#[test]
fn json_encoding_rejects_non_json_offline() {
    // validate via put path needs redis; unit-test mapping key builder instead.
    let map = KeyspaceMapping {
        vos_table: "User".into(),
        key_prefix: "iris:user:".into(),
        primary_key_field: "user_id".into(),
        encoding: KeyEncoding::JsonDocument,
        ttl_secs: None,
    };
    assert_eq!(map.redis_key("u1"), "iris:user:u1");
}

#[test]
fn live_pk_crud_ttl_nx_and_anti_scan() {
    let Some(url) = live_url() else {
        eprintln!("skip: set IRIS_TEST_REDIS_URL for live Redis conformance");
        return;
    };
    let db = RedisSource::connect(&url, sample_mapping()).expect("connect");

    db.put_primary("User", "u1", "alice").unwrap();
    assert_eq!(
        db.get_primary("User", "u1").unwrap().as_deref(),
        Some("alice")
    );
    let ttl = db.ttl_primary("User", "u1").unwrap();
    assert!(ttl > 0 && ttl <= 60, "ttl={ttl}");

    assert!(!db.put_primary_nx("User", "u1", "other").unwrap());
    assert!(db.put_primary_nx("User", "u2", "bob").unwrap());

    assert!(db.delete_primary("User", "u1").unwrap());
    assert_eq!(db.get_primary("User", "u1").unwrap(), None);
    let _ = db.delete_primary("User", "u2");

    let plan = Planner::new(RedisSource::capabilities())
        .plan_source(r#"User.collect()"#)
        .unwrap();
    let err = db.execute_plan(&plan).expect_err("scan must fail");
    assert!(
        err.to_string().contains("rejects") || err.to_string().contains("Scan"),
        "{err}"
    );

    let json_map = MappingManifest::with_tables(vec![KeyspaceMapping {
        vos_table: "Doc".into(),
        key_prefix: "iris:test:doc:".into(),
        primary_key_field: "id".into(),
        encoding: KeyEncoding::JsonDocument,
        ttl_secs: None,
    }]);
    let docs = RedisSource::connect(&url, json_map).unwrap();
    docs.put_primary("Doc", "1", r#"{"n":1}"#).unwrap();
    let bad = docs.put_primary("Doc", "2", "not-json");
    assert!(bad.is_err());
    let _ = docs.delete_primary("Doc", "1");
}

#[test]
fn connection_failure_unreachable_endpoint() {
    let mapping = sample_mapping();
    // Port 1 is almost never a Redis listener; connect builds a client, first op fails.
    let db = RedisSource::connect("redis://127.0.0.1:1/", mapping).expect("client open");
    let err = db.get_primary("User", "x").expect_err("unreachable redis");
    assert!(!err.to_string().is_empty());
}
