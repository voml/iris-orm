//! Phase 10-C: Redis Cache watermark / invalidate / RYW / BoundedStale / 回源.

use iris_adapter_redis::{
    CacheEntry, IdentityCacheResult, KeyEncoding, KeyspaceMapping, MappingManifest, RedisSource,
};
use iris_ir::{CommitToken, ConsistencyIntent, OutboxEffect, OutboxRecord};
use iris_types::{AppliedWatermarkState, StampedeBudget, StampedePermit};

fn sample_mapping() -> MappingManifest {
    MappingManifest::with_tables(vec![KeyspaceMapping {
        vos_table: "User".into(),
        key_prefix: "iris:test:cache10c:user:".into(),
        primary_key_field: "user_id".into(),
        encoding: KeyEncoding::Utf8String,
        ttl_secs: Some(120),
    }])
}

fn live_url() -> Option<String> {
    std::env::var("IRIS_TEST_REDIS_URL")
        .ok()
        .filter(|s| !s.is_empty())
}

fn unique_pk(prefix: &str) -> String {
    format!(
        "{prefix}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}

#[test]
fn stampede_budget_offline() {
    let budget = StampedeBudget::new(2);
    let a = StampedePermit::try_acquire(&budget).unwrap();
    let b = StampedePermit::try_acquire(&budget).unwrap();
    assert!(StampedePermit::try_acquire(&budget).is_none());
    drop(a);
    assert!(StampedePermit::try_acquire(&budget).is_some());
    drop(b);
}

#[test]
fn live_watermark_ryw_bounded_stale_and_outbox_apply() {
    let Some(url) = live_url() else {
        eprintln!("skip: set IRIS_TEST_REDIS_URL for Phase 10-C live cache test");
        return;
    };
    let src = RedisSource::connect(&url, sample_mapping()).expect("connect");
    let pk = unique_pk("u");
    let now = 1_700_000_000_000u64;

    // Unknown watermark ???BoundedStale must bypass (cannot prove lag).
    let bypass = src
        .cache_identity_read(
            "User",
            &pk,
            &ConsistencyIntent::BoundedStale { max_lag_secs: 60 },
            None,
            now,
            None,
        )
        .unwrap();
    assert!(
        matches!(
            bypass,
            IdentityCacheResult::BypassAuthority {
                reason: "bounded_stale_unknown_watermark",
                ..
            }
        ),
        "{bypass:?}"
    );

    // Projector applies outbox ???watermark + entry.
    let record = OutboxRecord {
        id: 1,
        commit_token: CommitToken::new(7),
        operation_id: format!("op-{pk}"),
        table: "User".into(),
        entity_id: pk.clone(),
        entity_version: 1,
        effect: OutboxEffect::Upsert,
    };
    let wm = src
        .cache_apply_outbox(&record, Some(r#"{"name":"ada"}"#), now)
        .unwrap();
    assert_eq!(wm.watermark.seq, 7);
    assert_eq!(wm.applied_unix_ms, now);
    let entry = src.cache_get_entry("User", &pk).unwrap().expect("entry");
    assert_eq!(entry.entity_version, 1);
    assert_eq!(entry.payload, r#"{"name":"ada"}"#);

    // BoundedStale within lag ???UseCache hit.
    let hit = src
        .cache_identity_read(
            "User",
            &pk,
            &ConsistencyIntent::BoundedStale { max_lag_secs: 30 },
            None,
            now + 5_000,
            None,
        )
        .unwrap();
    match hit {
        IdentityCacheResult::Hit {
            entry: Some(e),
            freshness_proven: true,
            ..
        } => assert_eq!(e.payload, r#"{"name":"ada"}"#),
        other => panic!("expected proven hit, got {other:?}"),
    }

    // RYW with higher fence ???bypass (回源 Authority).
    let fence = CommitToken::new(9);
    let ryw = src
        .cache_identity_read(
            "User",
            &pk,
            &ConsistencyIntent::ReadYourWrites,
            Some(&fence),
            now + 5_000,
            None,
        )
        .unwrap();
    assert!(
        matches!(
            ryw,
            IdentityCacheResult::BypassAuthority {
                reason: "ryw_fence_not_covered",
                ..
            }
        ),
        "{ryw:?}"
    );

    // Advance watermark / fill from authority after 回源.
    let filled = src
        .cache_fill_from_authority(
            "User",
            &pk,
            &CacheEntry {
                entity_version: 2,
                payload: r#"{"name":"ada2"}"#.into(),
                at_seq: 9,
            },
            &fence,
            now + 6_000,
        )
        .unwrap();
    assert_eq!(filled.watermark.seq, 9);
    let ryw_ok = src
        .cache_identity_read(
            "User",
            &pk,
            &ConsistencyIntent::ReadYourWrites,
            Some(&fence),
            now + 6_000,
            None,
        )
        .unwrap();
    assert!(
        matches!(
            ryw_ok,
            IdentityCacheResult::Hit {
                freshness_proven: true,
                ..
            }
        ),
        "{ryw_ok:?}"
    );

    // Invalidate discards entry; watermark remains.
    assert!(src.cache_invalidate("User", &pk).unwrap());
    assert!(src.cache_get_entry("User", &pk).unwrap().is_none());
    let still = src.cache_watermark(None).unwrap().unwrap();
    assert_eq!(still.watermark.seq, 9);

    // Authoritative never uses cache even with watermark.
    let auth = src
        .cache_identity_read(
            "User",
            &pk,
            &ConsistencyIntent::Authoritative,
            None,
            now + 7_000,
            None,
        )
        .unwrap();
    assert!(
        matches!(
            auth,
            IdentityCacheResult::BypassAuthority {
                reason: "authoritative_requires_authority",
                ..
            }
        ),
        "{auth:?}"
    );

    // Idempotent late outbox must not regress entity_version.
    let late = OutboxRecord {
        id: 2,
        commit_token: CommitToken::new(10),
        operation_id: format!("op-late-{pk}"),
        table: "User".into(),
        entity_id: pk.clone(),
        entity_version: 1,
        effect: OutboxEffect::Upsert,
    };
    // Re-put v2 then apply late v1
    src.cache_put_entry(
        "User",
        &pk,
        &CacheEntry {
            entity_version: 2,
            payload: "keep".into(),
            at_seq: 9,
        },
    )
    .unwrap();
    src.cache_apply_outbox(&late, Some("stale"), now + 8_000)
        .unwrap();
    let kept = src.cache_get_entry("User", &pk).unwrap().unwrap();
    assert_eq!(kept.payload, "keep");
    assert_eq!(kept.entity_version, 2);
    assert_eq!(
        src.cache_watermark(None).unwrap().unwrap().watermark.seq,
        10
    );

    // Cleanup
    let _ = src.cache_invalidate("User", &pk);
    let _ = src.cache_set_watermark(&AppliedWatermarkState::new(0, 0));
}
