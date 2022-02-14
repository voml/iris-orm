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
fn typescript_emit_writes_generated_client_files() {
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
    assert!(dir.join("generated/db.ts").is_file());
    assert!(dir.join("generated/synthesize.ts").is_file());
    let db = std::fs::read_to_string(dir.join("generated/db.ts")).expect("read db.ts");
    assert!(db.contains("$query<T = unknown>"));
    assert!(db.contains("synthesizeCreate"));
    assert!(!db.contains("@yydb/iris/node"));
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
    assert!(
        paths
            .iter()
            .all(|path| path.starts_with(dir.join("generated")))
    );
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
