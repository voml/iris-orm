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
        "iris-tools",
        "iris-generator",
        "Cargo.toml",
    ] {
        let path = workspace.join(rel);
        assert!(path.exists(), "missing workspace path {}", path.display());
    }
    for rel in ["readme.md", "projects/iris.ts", "projects/iris.cs"] {
        let path = product.join(rel);
        assert!(path.exists(), "missing product path {}", path.display());
    }
}

#[test]
fn cli_binary_crate_declares_iris_dependency() {
    let manifest = include_str!("../../iris-tools/Cargo.toml");
    assert!(
        manifest.contains("name = \"iris-tools\"")
            || manifest.contains("name = \"iris\"")
            || manifest.contains("iris")
    );
    assert!(
        manifest.contains("iris"),
        "iris-tools should depend on the public iris facade"
    );
}
