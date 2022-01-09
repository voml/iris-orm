//! Facade smoke.

#[test]
fn version_is_present() {
    assert!(iris::version().contains('.'));
}

#[test]
fn runtime_opens_reference_session() {
    let iris = iris::Runtime::new().open_reference(iris::ReferenceStore::new());
    let plan = iris
        .session()
        .plan_vos("User.take(0).collect()")
        .expect("empty take plans");
    assert_eq!(plan.envelope.ir_version, iris::IrVersion::PHASE1);
}
