//! Phase 7 install / package smoke (no external services).

#[test]
fn public_facade_version_and_modules_resolve() {
    assert!(!iris::version().is_empty());
    let _ = iris_types::Runtime::new();
    let _ = iris_ir::vos_facade_name();
    assert_eq!(iris_ir::IrVersion::PHASE1.major, 0);
}

#[test]
fn workspace_crate_dirs_exist_for_clean_checkout() {
    use std::path::PathBuf;
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let product = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for rel in [
        "iris",
        "iris-types",
        "iris-ir",
        "iris-connector-yydb",
        "iris-connector-yyds",
        "iris-adapter-sqlite",
        "iris-adapter-postgres",
        "iris-adapter-mysql",
        "iris-adapter-redis",
        "iris-generator",
    ] {
        let path = workspace.join(rel);
        assert!(path.exists(), "missing workspace path {}", path.display());
    }
    for rel in [
        "Cargo.toml",
        "Readme.md",
        "projects/packages",
        "projects/packages/iris/package.json",
    ] {
        let path = product.join(rel);
        assert!(path.exists(), "missing product path {}", path.display());
    }
}

#[test]
fn node_cli_package_declares_iris_bin() {
    let manifest = include_str!("../../../packages/iris/package.json");
    assert!(
        manifest.contains("\"bin\"") && manifest.contains("\"iris\""),
        "@yydb/iris should ship the sole iris CLI entry"
    );
}
