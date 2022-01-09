//! Phase 1 conformance: interpreter vs planner, and spanned rejection.

use iris::{
    CapabilitySet, Iris, PhysicalOp, QueryCaps, RealizationClass, ReferenceStore, Runtime, Value,
    WriteCaps, row_from_pairs,
};

fn sample_users() -> ReferenceStore {
    let mut store = ReferenceStore::new();
    store.seed(
        "User",
        vec![
            row_from_pairs(&[
                ("user_id", Value::Str("1".into())),
                ("user_name", Value::Str("alice".into())),
                ("active", Value::Bool(true)),
            ]),
            row_from_pairs(&[
                ("user_id", Value::Str("2".into())),
                ("user_name", Value::Str("bob".into())),
                ("active", Value::Bool(false)),
            ]),
            row_from_pairs(&[
                ("user_id", Value::Str("3".into())),
                ("user_name", Value::Str("carol".into())),
                ("active", Value::Bool(true)),
            ]),
        ],
    );
    store
}

#[test]
fn interpreter_and_planner_agree_on_filter_sort_take() {
    let source = r#"
User.filter(x => x.active)
    .sort_by(x => x.user_name)
    .take(10)
    .collect()
"#;
    let store = sample_users();
    let iris = Runtime::new().open_reference(store);
    let session = iris.session();

    let via_interp = session.interpret_vos(source).expect("interpret");
    let via_plan = session.execute_vos(source).expect("execute plan");
    assert_eq!(via_interp, via_plan);
    assert_eq!(via_plan.len(), 2);
    assert_eq!(
        via_plan[0].get("user_name"),
        Some(&Value::Str("alice".into()))
    );
    assert_eq!(
        via_plan[1].get("user_name"),
        Some(&Value::Str("carol".into()))
    );

    let plan = session.plan_vos(source).expect("plan");
    assert!(!plan.is_rejected());
    assert!(
        plan.nodes
            .iter()
            .all(|n| n.realization == RealizationClass::Native)
    );
    assert!(matches!(
        plan.nodes.last().map(|n| &n.op),
        Some(PhysicalOp::Collect)
    ));
}

#[test]
fn let_bound_pipeline_matches_direct() {
    let source = r#"
let users = User::filter(x => x.active)
    .sort_by(x => x.user_name)
    .take(20)
    .collect()
users
"#;
    let iris = Runtime::new().open_reference(sample_users());
    let session = iris.session();
    let a = session.interpret_vos(source).unwrap();
    let b = session.execute_vos(source).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.len(), 2);
}

#[test]
fn unsupported_filter_rejected_before_execute_with_span() {
    let mut caps = CapabilitySet::reference_full();
    caps.query = QueryCaps::scan_only();
    caps.write = WriteCaps::none();

    let iris = Iris::new(caps, sample_users());
    let session = iris.session();
    let source = "User.filter(x => x.active).collect()";
    let err = session.execute_vos(source).expect_err("must reject");
    let span = err.span().expect("spanned diagnostic");
    assert!(span.end >= span.start);
    let msg = err.to_string();
    assert!(
        msg.contains("filter") || msg.contains("rejected") || msg.contains("cannot"),
        "msg={msg}"
    );
}

#[test]
fn write_method_rejected_at_lower_with_span() {
    let iris = Runtime::new().open_reference(sample_users());
    let err = iris
        .session()
        .execute_vos("User.filter(x => x.active).delete()")
        .expect_err("writes rejected");
    assert!(err.span().is_some());
}

#[test]
fn version_and_envelope_are_phase1() {
    let iris = Runtime::new().open_reference(sample_users());
    let plan = iris
        .session()
        .plan_vos("User.take(1).collect()")
        .expect("plan");
    assert_eq!(plan.envelope.ir_version, iris::IrVersion::PHASE1);
}
