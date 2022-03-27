//! TypeScript client generation smoke (same GenerationModel as Rust emit).

use std::path::Path;

use iris_generator::{GenerationModel, write_typescript_client};

const USER_SCHEMA: &str = r#"
table User {
    @@user_id: uuid,
    user_name: utf8,
    active: bool,
}
"#;

#[test]
fn typescript_emit_writes_ux_layout() {
    let model = GenerationModel::from_vos_schema(USER_SCHEMA).expect("schema");
    let dir = std::env::temp_dir().join(format!(
        "iris-ts-gen-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let paths = write_typescript_client(&model, &dir).expect("write");
    assert_eq!(paths.len(), 10);
    let root = dir.join("generated/iris/typescript");
    assert!(root.join("index.ts").is_file());
    assert!(root.join("models.ts").is_file());
    assert!(root.join("operations.ts").is_file());
    assert!(root.join("metadata.ts").is_file());
    assert!(root.join("errors.ts").is_file());
    assert!(root.join("_internal/synthesize.ts").is_file());
    assert!(!root.join("synthesize.ts").is_file());
    assert!(!root.join("db.ts").is_file());
    let ops = std::fs::read_to_string(root.join("operations.ts")).expect("read operations");
    assert!(ops.contains("$query<T = unknown>"));
    assert!(ops.contains("synthesizeCreate"));
    assert!(ops.contains("./_internal/synthesize.js"));
    assert!(!ops.contains("@yydb/iris/node"));
    let index = std::fs::read_to_string(root.join("index.ts")).expect("read index");
    assert!(index.contains("./operations.js"));
    assert!(!index.contains("synthesize"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn generate_dispatch_typescript_target() {
    let dir = std::env::temp_dir().join(format!(
        "iris-ts-dispatch-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let (_, paths) =
        iris_generator::generate_from_source(USER_SCHEMA, "typescript", &dir).expect("generate");
    assert_eq!(paths.len(), 10);
    assert!(paths.iter().all(|path| {
        path.starts_with(dir.join("generated/iris/typescript"))
    }));
    assert!(
        paths
            .iter()
            .any(|path| path.file_name().is_some_and(|name| name == "metadata.ts"))
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn generate_rejects_unknown_target() {
    let err =
        iris_generator::generate_from_source(USER_SCHEMA, "kotlin", Path::new(".")).unwrap_err();
    assert!(err.to_string().contains("kotlin"));
}
