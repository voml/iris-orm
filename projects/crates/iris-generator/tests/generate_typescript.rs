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

const BLOG_SCHEMA: &str = r#"
table User {
    @@user_id: uuid,
    user_name: utf8,
    active: bool,
}

table Post {
    @@post_id: uuid,
    author: &User,
    title: utf8,
    published: bool,
}
"#;

fn assert_compact_ts_layout(name: &str, content: &str) {
    assert!(!content.starts_with('\n'), "{name} must not start with a blank line");
    assert!(!content.contains("\n\n"), "{name} must not contain consecutive blank lines");
    assert!(content.ends_with('\n'), "{name} must end with a single newline");
    assert!(!content.contains("import type {\n\n"), "{name} must not have blank lines inside import type braces");
    for entity in ["&quot;", "&lt;", "&gt;", "&amp;", "&#39;"] {
        assert!(!content.contains(entity), "{name} must not contain HTML entity {entity}");
    }
}

#[test]
fn typescript_emit_writes_ux_layout() {
    let model = GenerationModel::from_vos_schema(USER_SCHEMA).expect("schema");
    let dir = std::env::temp_dir()
        .join(format!("iris-ts-gen-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let root = dir.join("generated/iris");
    let paths = write_typescript_client(&model, &root).expect("write");
    assert_eq!(paths.len(), 12);
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
    let dir = std::env::temp_dir()
        .join(format!("iris-ts-dispatch-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let root = dir.join("generated/iris");
    let (_, paths) = iris_generator::generate_from_source(USER_SCHEMA, "typescript", &root).expect("generate");
    assert_eq!(paths.len(), 12);
    assert!(paths.iter().all(|path| path.starts_with(&root)));
    assert!(paths.iter().any(|path| path.file_name().is_some_and(|name| name == "metadata.ts")));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn typescript_emit_has_typed_filters_patch_and_payload() {
    let model = GenerationModel::from_vos_schema(USER_SCHEMA).expect("schema");
    let files = iris_generator::emit_typescript_client(&model).expect("emit");
    let inputs = files.iter().find(|(name, _)| name == "inputs.ts").map(|(_, content)| content.as_str()).expect("inputs.ts");
    assert!(inputs.contains("export type StringFilter"));
    assert!(inputs.contains("export type BooleanFilter"));
    assert!(inputs.contains("export type PatchValue<"));
    assert!(inputs.contains("UserPatchInput"));
    assert!(!inputs.contains("UpdateInput"));
    assert!(inputs.contains("UserGetPayload<"));

    let ops = files.iter().find(|(name, _)| name == "operations.ts").map(|(_, content)| content.as_str()).expect("operations.ts");
    assert!(ops.contains("findMany<const A extends UserFindManyArgs>"));
    assert!(ops.contains("ReadonlyArray<UserGetPayload<A>>"));
    assert!(
        ops.contains("} from \"./inputs.js\";\n/** Schema macros"),
        "single newline between imports and GeneratedMacros when macros are empty"
    );
    assert!(inputs.contains("active?: boolean | BooleanFilter"));
    assert!(inputs.contains("userName?: string | StringFilter"));
    assert!(inputs.contains("userId: UserId"));
    assert!(!inputs.contains("user_name?:"));
    assert!(!inputs.contains("&quot;"));
    assert!(!inputs.contains("&lt;"));
}

#[test]
fn typescript_emit_compact_layout_for_all_files() {
    let model = GenerationModel::from_vos_schema(BLOG_SCHEMA).expect("schema");
    let files = iris_generator::emit_typescript_client(&model).expect("emit");
    assert_eq!(files.len(), 12);
    for (name, content) in &files {
        assert_compact_ts_layout(name, content);
    }
}

#[test]
fn typescript_emit_unescapes_conditional_types() {
    let model = GenerationModel::from_vos_schema(USER_SCHEMA).expect("schema");
    let files = iris_generator::emit_typescript_client(&model).expect("emit");
    let inputs = files.iter().find(|(name, _)| name == "inputs.ts").map(|(_, content)| content.as_str()).expect("inputs.ts");
    assert!(inputs.contains("type EntityName = \"User\""));
    assert!(inputs.contains("[S] extends [undefined]"));
}

#[test]
fn typescript_emit_maps_wire_names_in_metadata() {
    let model = GenerationModel::from_vos_schema(BLOG_SCHEMA).expect("schema");
    let files = iris_generator::emit_typescript_client(&model).expect("emit");
    let metadata = files.iter().find(|(name, _)| name == "metadata.ts").map(|(_, content)| content.as_str()).expect("metadata.ts");
    assert!(metadata.contains("userName: \"user_name\""));
    assert!(metadata.contains("postId: \"post_id\""));
}

#[test]
fn typescript_emit_rejects_field_naming_collision() {
    const COLLISION_SCHEMA: &str = r#"
table User {
    @@id: uuid,
    user_id: uuid,
    user_Id: uuid,
}
"#;
    let model = GenerationModel::from_vos_schema(COLLISION_SCHEMA).expect("schema");
    let err = iris_generator::emit_typescript_client(&model).unwrap_err();
    let message = err.to_string();
    assert!(message.contains("collision") || message.contains("NamingCollision"));
}

#[test]
fn generate_rejects_unknown_target() {
    let err = iris_generator::generate_from_source(USER_SCHEMA, "kotlin", Path::new(".")).unwrap_err();
    assert!(err.to_string().contains("kotlin"));
}
