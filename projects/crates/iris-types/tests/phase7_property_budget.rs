//! Phase 7: property tests, budget enforcement, microbench smoke.

use iris_ir::{IrVersion, PhysicalOp, hash_ops};
use iris_types::{CompensationBudget, Planner, ReferenceStore, Runtime, Value, row_from_pairs};

#[test]
fn property_ir_version_accepts_iff_major_match_and_minor_le() {
    for major in 0u16..4 {
        for supported_minor in 0u16..6 {
            let supported = IrVersion {
                major,
                minor: supported_minor,
            };
            for other_major in 0u16..4 {
                for other_minor in 0u16..6 {
                    let other = IrVersion {
                        major: other_major,
                        minor: other_minor,
                    };
                    let expected = major == other_major && other_minor <= supported_minor;
                    assert_eq!(
                        supported.accepts(other),
                        expected,
                        "supported={supported:?} other={other:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn property_semantic_hash_stable_under_clone_and_sensitive_to_ops() {
    let bases: Vec<Vec<PhysicalOp>> = vec![
        vec![PhysicalOp::Scan { table: "A".into() }, PhysicalOp::Collect],
        vec![
            PhysicalOp::Scan { table: "A".into() },
            PhysicalOp::Take { count: 3 },
            PhysicalOp::Collect,
        ],
        vec![
            PhysicalOp::Scan { table: "B".into() },
            PhysicalOp::Skip { count: 1 },
            PhysicalOp::Take { count: 2 },
            PhysicalOp::Collect,
        ],
    ];
    for ops in &bases {
        assert_eq!(hash_ops(ops), hash_ops(&ops.clone()));
    }
    assert_ne!(hash_ops(&bases[0]), hash_ops(&bases[1]));
    assert_ne!(hash_ops(&bases[1]), hash_ops(&bases[2]));
}

#[test]
fn compensation_budget_rejects_oversized_result() {
    let mut store = ReferenceStore::new();
    let rows = (0..5)
        .map(|i| row_from_pairs(&[("id", Value::Int(i)), ("name", Value::Str(format!("n{i}")))]))
        .collect();
    store.seed("User", rows);

    let plan = Planner::new(Default::default())
        .plan_source(r#"User.collect()"#)
        .unwrap();
    let tight = CompensationBudget {
        max_rows: 2,
        ..CompensationBudget::default()
    };
    let err = store
        .execute_plan_with_budget(&plan, Some(&tight))
        .expect_err("budget");
    assert!(
        err.to_string().contains("compensation budget exceeded"),
        "{err}"
    );

    let ok = store
        .execute_plan_with_budget(&plan, Some(&CompensationBudget::default()))
        .unwrap();
    assert_eq!(ok.len(), 5);
}

#[test]
fn microbench_plan_and_reference_execute_under_budget_millis() {
    let mut store = ReferenceStore::new();
    let rows = (0..200)
        .map(|i| {
            row_from_pairs(&[
                ("id", Value::Int(i)),
                ("active", Value::Bool(i % 2 == 0)),
                ("name", Value::Str(format!("user-{i}"))),
            ])
        })
        .collect();
    store.seed("User", rows);

    let started = std::time::Instant::now();
    let plan = Planner::new(Default::default())
        .plan_source(
            r#"
            User.filter(x => x.active)
                .sort_by(x => x.name)
                .take(20)
                .collect()
            "#,
        )
        .unwrap();
    let out = store.execute_plan(&plan).unwrap();
    let elapsed = started.elapsed();
    assert_eq!(out.len(), 20);
    // Smoke ceiling only -- keeps CI honest without being a flaky perf gate.
    assert!(
        elapsed.as_millis() < CompensationBudget::default().max_millis as u128,
        "elapsed={elapsed:?}"
    );
}

#[test]
fn runtime_smoke_opens_reference_session() {
    let mut store = ReferenceStore::new();
    store.seed("T", vec![row_from_pairs(&[("k", Value::Str("v".into()))])]);
    let iris = Runtime::new().open_reference(store);
    let _ = iris.session();
}
