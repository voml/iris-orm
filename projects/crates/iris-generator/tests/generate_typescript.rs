//! TypeScript client generation smoke (same GenerationModel as Rust emit).

use std::path::Path;

use iris_generator::{GenerationModel, write_typescript_client};
use vos::{FieldIdentity, IDENTITY_MANIFEST_VERSION, IdentityManifest, TypeContractKind, TypeIdentity, parse_oak, resolve_contract};

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

const CARRIER_SCHEMA: &str = r#"
table Metric {
    @@metric_id: uuid,
    seq: i64,
    counter: u64,
    note: utf16,
    payload: bytes,
    seen_at: datetime,
    scored_on: date,
}
"#;

#[test]
fn typescript_emit_keeps_vos_types_distinct_through_carriers() {
    let model = GenerationModel::from_vos_schema(CARRIER_SCHEMA).expect("schema");
    let files = iris_generator::emit_typescript_client(&model).expect("emit");
    let file = |name: &str| files.iter().find(|(path, _)| path == name).map(|(_, content)| content.as_str()).expect(name);

    let models = file("models.ts");
    assert!(models.contains("seq: bigint"), "{models}");
    assert!(models.contains("counter: bigint"), "{models}");
    assert!(models.contains("payload: Uint8Array"), "{models}");
    assert!(models.contains("note: string"), "{models}");
    assert!(models.contains("seenAt: string"), "{models}");
    assert!(models.contains("scoredOn: string"), "{models}");
    assert!(!models.contains("seq: number"), "`i64` must not collapse into a JS number: {models}");

    let inputs = file("inputs.ts");
    assert!(inputs.contains("export type BigIntFilter"), "{inputs}");
    assert!(inputs.contains("seq?: bigint | BigIntFilter;"), "{inputs}");
    assert!(inputs.contains("payload?: Uint8Array | BytesFilter;"), "{inputs}");
    assert!(inputs.contains("readonly eq?: Uint8Array"), "{inputs}");

    let metadata = file("metadata.ts");
    assert!(metadata.contains("seq: \"i64\""), "{metadata}");
    assert!(metadata.contains("counter: \"u64\""), "{metadata}");
    assert!(metadata.contains("note: \"utf16\""), "`utf16` must stay distinct from `utf8`: {metadata}");
    assert!(metadata.contains("payload: \"bytes\""), "{metadata}");
    assert!(metadata.contains("seenAt: \"datetimeUtc\""), "{metadata}");
    assert!(metadata.contains("scoredOn: \"date\""), "`date` must stay distinct from `datetime`: {metadata}");
}

#[test]
fn resolved_contract_through_generator_to_typescript_emitter() {
    // Full chain: VOS source → ResolvedContract → GenerationModel → emit_typescript_client.
    let source = r#"
table User {
    @@user_id: uuid,
    user_name: utf8,
    active: bool,
}

table Post {
    @@post_id: uuid,
    author: &User,
    title: utf8,
}
"#;
    let projection = parse_oak(source).unwrap().project_schema().unwrap();
    let manifest = IdentityManifest {
        format_version: IDENTITY_MANIFEST_VERSION.to_owned(),
        types: vec![
            TypeIdentity {
                canonical_path: vec!["User".to_owned()],
                type_id: 10,
                kind: TypeContractKind::Table,
                fields: vec![
                    FieldIdentity { canonical_name: "user_id".to_owned(), field_id: 101, virtual_field_index: Some(0) },
                    FieldIdentity { canonical_name: "user_name".to_owned(), field_id: 102, virtual_field_index: Some(1) },
                    FieldIdentity { canonical_name: "active".to_owned(), field_id: 103, virtual_field_index: Some(2) },
                ],
            },
            TypeIdentity {
                canonical_path: vec!["Post".to_owned()],
                type_id: 20,
                kind: TypeContractKind::Table,
                fields: vec![
                    FieldIdentity { canonical_name: "post_id".to_owned(), field_id: 201, virtual_field_index: Some(0) },
                    FieldIdentity { canonical_name: "author".to_owned(), field_id: 202, virtual_field_index: Some(1) },
                    FieldIdentity { canonical_name: "title".to_owned(), field_id: 203, virtual_field_index: Some(2) },
                ],
            },
        ],
    };
    let contract = resolve_contract(&projection, &manifest).expect("resolve");
    let artifact = contract.to_json().expect("serialize");
    let model = GenerationModel::from_resolved_artifact(&artifact).expect("from resolved");

    assert_eq!(model.tables.len(), 2);
    assert_eq!(model.tables[0].type_id, Some(10));
    assert_eq!(model.tables[1].type_id, Some(20));

    let files = iris_generator::emit_typescript_client(&model).expect("emit");
    let file = |name: &str| files.iter().find(|(path, _)| path == name).map(|(_, content)| content.as_str()).expect(name);

    let models = file("models.ts");
    assert!(models.contains("export interface User"), "{models}");
    assert!(models.contains("userName: string"), "{models}");
    assert!(models.contains("active: boolean"), "{models}");
    assert!(models.contains("export interface Post"), "{models}");
    assert!(models.contains("title: string"), "{models}");

    let refs = file("references.ts");
    assert!(refs.contains("export type UserId = string"), "{refs}");
    assert!(refs.contains("export type PostId = string"), "{refs}");

    let inputs = file("inputs.ts");
    assert!(inputs.contains("UserWhere"));
    assert!(inputs.contains("PostWhere"));

    let metadata = file("metadata.ts");
    assert!(metadata.contains("userName: \"user_name\""));
    assert!(metadata.contains("userId: \"user_id\""));
    assert!(metadata.contains("postId: \"post_id\""));

    let ops = file("operations.ts");
    assert!(ops.contains("findMany<const A extends UserFindManyArgs>"));
    assert!(ops.contains("findMany<const A extends PostFindManyArgs>"));
}

