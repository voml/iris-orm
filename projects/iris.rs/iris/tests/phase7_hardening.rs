//! Phase 7 hardening slice: IR versioning, adapter matrix, secret-leak guards.

use iris_ir::{
    EffectKind, IrEnvelope, IrVersion, PhysicalOp, SchemaFingerprint, SemanticHash, hash_ops,
};
use iris_types::{CapabilitySet, Planner, QueryCaps, WriteCaps};

#[test]
fn ir_version_accepts_same_major_lower_or_equal_minor() {
    let supported = IrVersion { major: 0, minor: 2 };
    assert!(supported.accepts(IrVersion { major: 0, minor: 1 }));
    assert!(supported.accepts(IrVersion { major: 0, minor: 2 }));
    assert!(!supported.accepts(IrVersion { major: 0, minor: 3 }));
    assert!(!supported.accepts(IrVersion { major: 1, minor: 0 }));
}

#[test]
fn envelope_rejects_newer_minor_and_other_major() {
    let mut env = sample_envelope(IrVersion { major: 0, minor: 2 });
    let err = env
        .check_version(IrVersion::PHASE1)
        .expect_err("newer minor");
    assert!(matches!(err, iris_ir::Error::UnsupportedVersion(0, 2)));

    env.ir_version = IrVersion { major: 1, minor: 0 };
    let err = env
        .check_version(IrVersion::PHASE1)
        .expect_err("other major");
    assert!(matches!(err, iris_ir::Error::UnsupportedVersion(1, 0)));
}

#[test]
fn semantic_hash_is_deterministic_for_same_ops() {
    let ops = vec![
        PhysicalOp::Scan {
            table: "User".into(),
        },
        PhysicalOp::Collect,
    ];
    assert_eq!(hash_ops(&ops), hash_ops(&ops));
    let mut other = ops.clone();
    other.insert(1, PhysicalOp::Take { count: 1 });
    assert_ne!(hash_ops(&ops), hash_ops(&other));
}

#[test]
fn adapter_compatibility_matrix_documents_phase_backends() {
    // Living matrix for Phase 7 --?keep in sync when backends gain capabilities.
    let matrix: &[(&str, CapabilitySet, &[&str])] = &[
        (
            "reference",
            CapabilitySet::reference_full(),
            &["scan", "filter", "sort", "page", "project"],
        ),
        (
            "yydb",
            CapabilitySet {
                backend_id: "yydb".into(),
                backend_version: "test".into(),
                ir_version_max: IrVersion::PHASE1,
                query: QueryCaps::full(),
                write: WriteCaps::full(),
                budget: Default::default(),
            },
            &["scan", "filter", "sort", "page", "project", "write"],
        ),
        (
            "sqlite",
            CapabilitySet {
                backend_id: "sqlite".into(),
                backend_version: "test".into(),
                ir_version_max: IrVersion::PHASE1,
                query: QueryCaps::full(),
                write: WriteCaps::full(),
                budget: Default::default(),
            },
            &["scan", "filter", "sort", "page", "project", "write"],
        ),
        (
            "postgres",
            CapabilitySet {
                backend_id: "postgres".into(),
                backend_version: "test".into(),
                ir_version_max: IrVersion::PHASE1,
                query: QueryCaps::full(),
                write: WriteCaps::full(),
                budget: Default::default(),
            },
            &["scan", "filter", "sort", "page", "project", "write"],
        ),
        (
            "mysql",
            CapabilitySet {
                backend_id: "mysql".into(),
                backend_version: "test".into(),
                ir_version_max: IrVersion::PHASE1,
                query: QueryCaps::full(),
                write: WriteCaps::full(),
                budget: Default::default(),
            },
            &["scan", "filter", "sort", "page", "project", "write"],
        ),
        (
            "redis",
            CapabilitySet {
                backend_id: "redis".into(),
                backend_version: "test".into(),
                ir_version_max: IrVersion::PHASE1,
                query: QueryCaps::scan_only(),
                write: WriteCaps::full(),
                budget: Default::default(),
            },
            &["scan", "write"],
        ),
        (
            "yyds",
            CapabilitySet {
                backend_id: "yyds".into(),
                backend_version: "test".into(),
                ir_version_max: IrVersion::PHASE1,
                query: QueryCaps::full(),
                write: WriteCaps::full(),
                budget: Default::default(),
            },
            &["gated"], // not executable until YYDS VOS readiness clears
        ),
    ];

    for (id, caps, expected) in matrix {
        assert_eq!(caps.backend_id, *id);
        if expected.contains(&"filter") {
            assert!(caps.query.filter_cmp || caps.query.filter_bool, "{id}");
        } else if *id == "redis" {
            assert!(!caps.query.filter_cmp && !caps.query.sort, "{id}");
            let err = Planner::new(caps.clone())
                .plan_source(r#"User.filter(x => x.active).collect()"#)
                .expect_err("redis filter");
            let text = format!("{err:?}");
            assert!(
                text.contains("IRIS-PLAN-REJECTED")
                    || text.contains("reject")
                    || text.contains("filter")
                    || text.contains("capability"),
                "{text}"
            );
        }
        if expected.contains(&"gated") {
            assert_eq!(*id, "yyds");
        }
    }
}

#[test]
fn diagnostics_and_public_sources_do_not_embed_credential_placeholders() {
    // Plan a rejected redis-style query and ensure the diagnostic text has no
    // password / connection-string shaped material (even if a caller later
    // attaches a sanitized cause).
    let caps = CapabilitySet {
        backend_id: "redis".into(),
        backend_version: "test".into(),
        ir_version_max: IrVersion::PHASE1,
        query: QueryCaps::scan_only(),
        write: WriteCaps::none(),
        budget: Default::default(),
    };
    let err = Planner::new(caps)
        .plan_source(r#"User.filter(x => x.active).collect()"#)
        .unwrap_err();
    let text = format!("{err:?}");
    for banned in [
        "password=",
        "PASSWORD",
        "redis://:secret@",
        "mysql://user:pass@",
        "postgres://user:pass@",
        "secret_key",
        "BEGIN RSA PRIVATE",
    ] {
        assert!(
            !text.contains(banned),
            "diagnostic debug leaked credential marker `{banned}`: {text}"
        );
    }
}

#[test]
fn public_crate_sources_forbid_hardcoded_secret_assignments() {
    use std::fs;
    use std::path::{Path, PathBuf};

    fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if entry.file_name() == "target" {
                    continue;
                }
                collect(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for dir in [
        "iris/src",
        "iris-types/src",
        "iris-ir/src",
        "iris-connector-yydb/src",
        "iris-connector-yyds/src",
        "iris-adapter-sqlite/src",
        "iris-adapter-postgres/src",
        "iris-adapter-mysql/src",
        "iris-adapter-redis/src",
    ] {
        collect(&root.join(dir), &mut files);
    }

    let patterns = [
        "password = \"",
        "PASSWORD = \"",
        "secret_key: b\"",
        "api_key = \"",
        "private_key = \"",
    ];
    for path in files {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        for pat in patterns {
            assert!(
                !text.contains(pat),
                "{} must not hardcode secret assignment `{pat}`",
                path.display()
            );
        }
    }
}

fn sample_envelope(ir_version: IrVersion) -> IrEnvelope {
    IrEnvelope {
        vos_contract_version: "vos-dev".into(),
        ir_version,
        schema_fingerprint: SchemaFingerprint::unbound(),
        operation_id: "op-test".into(),
        effect: EffectKind::Read,
        required_capabilities: vec!["scan".into()],
        span_start: 0,
        span_end: 0,
        semantic_hash: SemanticHash(0),
    }
}
