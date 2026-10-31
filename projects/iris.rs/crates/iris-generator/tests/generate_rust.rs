//! Phase 8: Dejavu generation golden + determinism.

use iris_generator::{GenerationModel, TEMPLATE_NAMES, emit_rust_domain, prefers_aot};
use vos::{FieldIdentity, IDENTITY_MANIFEST_VERSION, IdentityManifest, TypeContractKind, TypeIdentity, parse_oak, resolve_contract};

const USER_SCHEMA: &str = r#"
table User {
    @@user_id: utf8,
    @user_name: utf8,
    active: bool,
}
"#;

#[test]
fn prefers_aot_by_default() {
    assert!(prefers_aot());
    assert!(TEMPLATE_NAMES.contains(&"rust/domain_mod"));
    assert!(TEMPLATE_NAMES.contains(&"rust/file_header"));
    assert!(TEMPLATE_NAMES.contains(&"rust/models_helpers"));
    assert!(TEMPLATE_NAMES.contains(&"rust/operations"));
    assert!(TEMPLATE_NAMES.contains(&"rust/metadata"));
}

#[test]
fn generation_model_from_vos_is_deterministic() {
    let a = GenerationModel::from_vos_schema(USER_SCHEMA).unwrap();
    let b = GenerationModel::from_vos_schema(USER_SCHEMA).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.tables.len(), 1);
    assert_eq!(a.tables[0].name, "User");
    assert!(a.tables[0].fields.iter().any(|f| f.primary));
    assert!(a.tables[0].fields.iter().any(|f| f.name == "active" && f.rust_ty == "bool"));
}

fn assert_canonical_fingerprint(fingerprint: &str) {
    assert_eq!(fingerprint.len(), 64, "fingerprint `{fingerprint}` must be a SHA-256 hex digest");
    assert!(fingerprint.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')), "fingerprint `{fingerprint}` must be lowercase hex");
}

#[test]
fn schema_fingerprint_is_content_addressed_and_stable() {
    let first = GenerationModel::from_vos_schema(USER_SCHEMA).unwrap().schema_fingerprint;
    let second = GenerationModel::from_vos_schema(USER_SCHEMA).unwrap().schema_fingerprint;
    assert_canonical_fingerprint(&first);
    assert_eq!(first, second);
}

#[test]
fn schema_fingerprint_ignores_declaration_and_field_order() {
    const REORDERED: &str = r#"
table User {
    active: bool,
    @user_name: utf8,
    @@user_id: utf8,
}
"#;
    let original = GenerationModel::from_vos_schema(USER_SCHEMA).unwrap().schema_fingerprint;
    let reordered = GenerationModel::from_vos_schema(REORDERED).unwrap().schema_fingerprint;
    assert_eq!(original, reordered);
}

#[test]
fn schema_fingerprint_tracks_semantic_changes() {
    const CHANGED_TYPE: &str = r#"
table User {
    @@user_id: utf8,
    @user_name: utf8,
    active: utf8,
}
"#;
    const REMOVED_FIELD: &str = r#"
table User {
    @@user_id: utf8,
    @user_name: utf8,
}
"#;
    let original = GenerationModel::from_vos_schema(USER_SCHEMA).unwrap().schema_fingerprint;
    let changed_type = GenerationModel::from_vos_schema(CHANGED_TYPE).unwrap().schema_fingerprint;
    let removed_field = GenerationModel::from_vos_schema(REMOVED_FIELD).unwrap().schema_fingerprint;
    assert_ne!(original, changed_type);
    assert_ne!(original, removed_field);
}

#[test]
fn generation_rejects_schema_without_a_table_primary() {
    assert!(GenerationModel::from_vos_schema("table User { id: utf8 }").is_err());
}

#[test]
fn generation_model_keeps_macros_next_to_tables() {
    let schema = r#"
table User {
    @@id: uuid,
    name: utf8,
}

macro seed_user(prefix: utf8) -> unit {
    User::insert({ id: "1", name: prefix })
}
"#;
    let model = GenerationModel::from_vos_schema(schema).unwrap();
    assert_eq!(model.tables.len(), 1);
    assert_eq!(model.macros.len(), 1);
    assert_eq!(model.macros[0].name, "seed_user");
    assert_eq!(model.macros[0].return_type, "unit");
    assert_eq!(model.macros[0].params[0].vos_type, "utf8");
    assert_eq!(model.macros[0].params[0].ts_type, "string");
}

#[test]
fn generation_model_consumes_resolved_contract_identity() {
    let projection = parse_oak("table User { @@id: uuid, active: bool }").unwrap().project_schema().unwrap();
    let manifest = IdentityManifest {
        format_version: IDENTITY_MANIFEST_VERSION.to_owned(),
        types: vec![TypeIdentity {
            canonical_path: vec!["User".to_owned()],
            type_id: 7,
            kind: TypeContractKind::Table,
            fields: vec![
                FieldIdentity { canonical_name: "id".to_owned(), field_id: 8, virtual_field_index: Some(0) },
                FieldIdentity { canonical_name: "active".to_owned(), field_id: 9, virtual_field_index: Some(1) },
            ],
        }],
    };
    let contract = resolve_contract(&projection, &manifest).unwrap();
    let model = GenerationModel::from_resolved_artifact(&contract.to_json().unwrap()).unwrap();
    assert_eq!(model.schema_fingerprint, contract.schema_fingerprint);
    assert_eq!(model.tables[0].type_id, Some(7));
    assert_eq!(model.tables[0].fields[0].field_id, Some(8));
    assert_eq!(model.tables[0].fields[0].virtual_field_index, Some(0));
    assert!(model.tables[0].fields[0].primary);
}

#[test]
fn rust_emit_contains_structs_fingerprint_and_no_sql() {
    let model = GenerationModel::from_vos_schema(USER_SCHEMA).unwrap();
    let out = emit_rust_domain(&model, "mysql").unwrap();
    assert!(out.contains("Generated by Iris"));
    assert!(out.contains("pub struct User"));
    assert!(out.contains("pub user_id: String"));
    assert!(out.contains("pub active: bool"));
    assert!(out.contains("SCHEMA_FINGERPRINT"));
    assert!(out.contains("UUID_FIELDS"));
    assert!(out.contains("pub struct Db"));
    assert!(out.contains("pub struct Txn"));
    assert!(out.contains("fn find_many"));
    assert!(out.contains("fn with_rollback"));
    assert!(out.contains("fn query"));
    assert!(out.contains("UserWhere"));
    assert!(out.contains(".filter(x =>"));
    assert!(!out.contains("SELECT "));
    assert!(!out.contains("CREATE TABLE"));
    let again = emit_rust_domain(&model, "mysql").unwrap();
    assert_eq!(out, again);
}

#[test]
fn rust_emit_optional_types_are_not_html_escaped() {
    let schema = r#"
table Item {
    @@id: utf8,
    note: utf8?,
}
"#;
    let model = GenerationModel::from_vos_schema(schema).unwrap();
    let out = emit_rust_domain(&model, "mysql").unwrap();
    assert!(out.contains("Option<String>"));
    assert!(!out.contains("&lt;"));
}

#[test]
fn rust_emit_uuid_fields_table() {
    let schema = r#"
table Item {
    @@id: uuid,
    owner_id: uuid?,
    name: utf8,
}
"#;
    let model = GenerationModel::from_vos_schema(schema).unwrap();
    assert!(model.tables[0].fields.iter().any(|f| f.name == "id" && f.is_uuid));
    assert!(model.tables[0].fields.iter().any(|f| f.name == "owner_id" && f.is_uuid && f.optional));
    let out = emit_rust_domain(&model, "mysql").unwrap();
    assert!(out.contains(r#"("Item", "id")"#));
    assert!(out.contains(r#"("Item", "owner_id")"#));
    assert!(!out.contains(r#"("Item", "name")"#));
}

#[test]
fn resolved_contract_through_generator_to_rust_emitter() {
    // Full chain: VOS source → ResolvedContract (identity-bound) → GenerationModel → emit_rust_domain.
    let source = r#"
table User {
    @@id: uuid,
    name: utf8,
    active: bool,
}
"#;
    let projection = parse_oak(source).unwrap().project_schema().unwrap();
    let manifest = IdentityManifest {
        format_version: IDENTITY_MANIFEST_VERSION.to_owned(),
        types: vec![TypeIdentity {
            canonical_path: vec!["User".to_owned()],
            type_id: 42,
            kind: TypeContractKind::Table,
            fields: vec![
                FieldIdentity { canonical_name: "id".to_owned(), field_id: 1, virtual_field_index: Some(0) },
                FieldIdentity { canonical_name: "name".to_owned(), field_id: 2, virtual_field_index: Some(1) },
                FieldIdentity { canonical_name: "active".to_owned(), field_id: 3, virtual_field_index: Some(2) },
            ],
        }],
    };
    let contract = resolve_contract(&projection, &manifest).expect("resolve");
    let artifact = contract.to_json().expect("serialize");
    let model = GenerationModel::from_resolved_artifact(&artifact).expect("from resolved");

    assert_eq!(model.tables.len(), 1);
    assert_eq!(model.tables[0].type_id, Some(42));
    assert_eq!(model.tables[0].fields[0].field_id, Some(1));
    assert_eq!(model.tables[0].fields[2].name, "active");
    assert_eq!(model.tables[0].fields[2].rust_ty, "bool");
    assert!(model.tables[0].fields[0].primary);
    // ResolvedContract path does not carry macro/query operations yet.
    assert!(model.macros.is_empty());

    // Feed the identity-bound model straight into the Rust emitter.
    let out = emit_rust_domain(&model, "mysql").expect("emit");
    assert!(out.contains("pub struct User"));
    assert!(out.contains("pub id: String"));
    assert!(out.contains("pub name: String"));
    assert!(out.contains("pub active: bool"));
    assert!(out.contains("SCHEMA_FINGERPRINT"));
    assert!(out.contains("UUID_FIELDS"));
    assert!(!out.contains("SELECT"));

    // Round-trip determinism: same contract produces same code.
    let again = emit_rust_domain(&model, "mysql").unwrap();
    assert_eq!(out, again);
}

#[test]
fn write_rust_domain_atomic() {
    let dir = std::env::temp_dir()
        .join(format!("iris-gen-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let model = GenerationModel::from_vos_schema(USER_SCHEMA).unwrap();
    let generate_root = dir.join("generated/iris");
    let paths = iris_generator::write_rust_domain(&model, "mysql", &generate_root).unwrap();
    let root = generate_root.join("rust");
    assert_eq!(paths.len(), 5);
    assert!(paths.iter().all(|p| p.starts_with(&root)));
    assert!(root.join("mod.rs").is_file());
    assert!(root.join("models.rs").is_file());
    assert!(root.join("operations.rs").is_file());
    assert!(root.join("metadata.rs").is_file());
    assert!(root.join("errors.rs").is_file());
    let text = std::fs::read_to_string(root.join("models.rs")).unwrap();
    assert!(text.contains("pub struct User"));
    let ops = std::fs::read_to_string(root.join("operations.rs")).unwrap();
    assert!(ops.contains("pub struct Db"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn from_vos_schema_maps_list_field_to_rust_vec() {
    let schema = r#"
table Tag {
    @@id: uuid,
    label: utf8,
}

table Item {
    @@id: uuid,
    tags: [utf8],
    names: [utf8?],
}
"#;
    let model = GenerationModel::from_vos_schema(schema).expect("parse schema");
    let item = model.tables.iter().find(|t| t.name == "Item").expect("Item table");
    let tags = item.fields.iter().find(|f| f.name == "tags").expect("tags field");
    assert_eq!(tags.rust_ty, "Vec<String>");
    assert_eq!(tags.vos_type, "[utf8]");
    assert!(!tags.optional);
    let names = item.fields.iter().find(|f| f.name == "names").expect("names field");
    assert_eq!(names.rust_ty, "Vec<Option<String>>");
    assert_eq!(names.vos_type, "[utf8?]");
    assert!(names.optional);
}

#[test]
fn from_resolved_artifact_maps_list_field_to_rust_vec() {
    let source = r#"
table Item {
    @@id: uuid,
    tags: [utf8],
}
"#;
    let projection = parse_oak(source).unwrap().project_schema().unwrap();
    let manifest = IdentityManifest {
        format_version: IDENTITY_MANIFEST_VERSION.to_owned(),
        types: vec![TypeIdentity {
            canonical_path: vec!["Item".to_owned()],
            type_id: 77,
            kind: TypeContractKind::Table,
            fields: vec![
                FieldIdentity { canonical_name: "id".to_owned(), field_id: 1, virtual_field_index: Some(0) },
                FieldIdentity { canonical_name: "tags".to_owned(), field_id: 2, virtual_field_index: Some(1) },
            ],
        }],
    };
    let contract = resolve_contract(&projection, &manifest).expect("resolve");
    let model = GenerationModel::from_resolved_artifact(&contract.to_json().unwrap()).expect("from resolved");
    let item = &model.tables[0];
    assert_eq!(item.type_id, Some(77));
    let tags = item.fields.iter().find(|f| f.name == "tags").expect("tags field");
    assert_eq!(tags.rust_ty, "Vec<String>");
    assert_eq!(tags.vos_type, "[utf8]");
    assert!(!tags.optional);
}

/// Minimal hard-coded V0 operation projection artifact with three ops:
/// query + udf + macro. JSON is hand-written so the test exercises the exact
/// serde shape Iris consumes (deny_unknown_fields, camelCase keys, span
/// wire format `{start, end}`).
const OPERATION_ARTIFACT_JSON: &str = r#"{
    "envelope": {
        "stage": "operation-syntax-projection",
        "contractFormatVersion": "vos-operation-contract-v0",
        "languageVersion": "vos-language-v0",
        "canonicalizationVersion": "vos-canonical-v0",
        "schemaFingerprint": null,
        "sourceUnits": [
            { "sourceUnitId": 0, "source": "query get_user(id: uuid) -> User?\nudf hash_password(raw: utf8) -> utf8\nmacro seed_user() -> unit" }
        ]
    },
    "operations": [
        {
            "kind": "query",
            "name": "get_user",
            "identityKey": "query:get_user(id:uuid)->User?",
            "operationId": "op_a1b2c3d4e5f67890",
            "parameters": [
                { "name": "id", "typeExpr": { "named": ["uuid"] }, "span": { "start": 15, "end": 23 } }
            ],
            "returnType": { "optional": { "named": ["User"] } },
            "signature": { "text": "get_user(id: uuid) -> User?", "span": { "start": 6, "end": 32 } },
            "body": null,
            "span": { "start": 0, "end": 45 }
        },
        {
            "kind": "udf",
            "name": "hash_password",
            "identityKey": "udf:hash_password(raw:utf8)->utf8",
            "operationId": "op_b2c3d4e5f6a78901",
            "parameters": [
                { "name": "raw", "typeExpr": { "named": ["utf8"] }, "span": { "start": 60, "end": 68 } }
            ],
            "returnType": { "named": ["utf8"] },
            "signature": { "text": "hash_password(raw: utf8) -> utf8", "span": { "start": 46, "end": 75 } },
            "body": null,
            "span": { "start": 45, "end": 90 }
        },
        {
            "kind": "macro",
            "name": "seed_user",
            "identityKey": "macro:seed_user()->unit",
            "operationId": "op_c3d4e5f6a7b89012",
            "parameters": [],
            "returnType": null,
            "signature": { "text": "seed_user() -> unit", "span": { "start": 92, "end": 112 } },
            "body": null,
            "span": { "start": 90, "end": 120 }
        }
    ]
}"#;

#[test]
fn from_artifacts_consumes_operations_and_macros() {
    // Build a ResolvedContract for the schema side (uses vos crate).
    let source = r#"
table User {
    @@id: uuid,
    name: utf8,
}
"#;
    let projection = parse_oak(source).unwrap().project_schema().unwrap();
    let manifest = IdentityManifest {
        format_version: IDENTITY_MANIFEST_VERSION.to_owned(),
        types: vec![TypeIdentity {
            canonical_path: vec!["User".to_owned()],
            type_id: 42,
            kind: TypeContractKind::Table,
            fields: vec![
                FieldIdentity { canonical_name: "id".to_owned(), field_id: 1, virtual_field_index: Some(0) },
                FieldIdentity { canonical_name: "name".to_owned(), field_id: 2, virtual_field_index: Some(1) },
            ],
        }],
    };
    let contract = resolve_contract(&projection, &manifest).expect("resolve");
    let contract_json = contract.to_json().expect("serialize contract");

    // Now feed BOTH artifacts to GenerationModel::from_artifacts.
    let model =
        GenerationModel::from_artifacts(&contract_json, OPERATION_ARTIFACT_JSON).expect("from_artifacts should parse contract + operations");

    // Schema side intact.
    assert_eq!(model.tables.len(), 1);
    assert_eq!(model.tables[0].type_id, Some(42));

    // Operations consumed and sorted by operation_id.
    assert_eq!(model.operations.len(), 3);
    let ids: Vec<&str> = model.operations.iter().map(|o| o.operation_id.as_str()).collect();
    assert_eq!(ids, vec!["op_a1b2c3d4e5f67890", "op_b2c3d4e5f6a78901", "op_c3d4e5f6a7b89012"]);

    let get_user = model.operations.iter().find(|o| o.name == "get_user").expect("get_user");
    assert_eq!(get_user.kind, "query");
    assert_eq!(get_user.params.len(), 1);
    assert_eq!(get_user.params[0].name, "id");
    assert_eq!(get_user.params[0].vos_type, "uuid");
    assert_eq!(get_user.params[0].ts_type, "string");
    assert_eq!(get_user.return_type.as_deref(), Some("User?"));

    let hash_password = model.operations.iter().find(|o| o.name == "hash_password").expect("hash_password");
    assert_eq!(hash_password.kind, "udf");
    assert_eq!(hash_password.return_type.as_deref(), Some("utf8"));

    let seed_user = model.operations.iter().find(|o| o.name == "seed_user").expect("seed_user");
    assert_eq!(seed_user.kind, "macro");
    assert!(seed_user.params.is_empty());
    assert!(seed_user.return_type.is_none());

    // Macro operations also populate `macros` so existing TS template keeps working.
    assert_eq!(model.macros.len(), 1);
    assert_eq!(model.macros[0].name, "seed_user");
    assert_eq!(model.macros[0].return_type, "unit");
}

#[test]
fn emit_rust_domain_includes_operation_stubs() {
    let source = r#"
table User {
    @@id: uuid,
    name: utf8,
}
"#;
    let projection = parse_oak(source).unwrap().project_schema().unwrap();
    let manifest = IdentityManifest {
        format_version: IDENTITY_MANIFEST_VERSION.to_owned(),
        types: vec![TypeIdentity {
            canonical_path: vec!["User".to_owned()],
            type_id: 42,
            kind: TypeContractKind::Table,
            fields: vec![
                FieldIdentity { canonical_name: "id".to_owned(), field_id: 1, virtual_field_index: Some(0) },
                FieldIdentity { canonical_name: "name".to_owned(), field_id: 2, virtual_field_index: Some(1) },
            ],
        }],
    };
    let contract = resolve_contract(&projection, &manifest).unwrap();
    let model = GenerationModel::from_artifacts(&contract.to_json().unwrap(), OPERATION_ARTIFACT_JSON).unwrap();

    let files = iris_generator::emit_rust_files(&model, "mysql").unwrap();
    let (_, ops_content) = files.iter().find(|(name, _)| name == "operations.rs").expect("operations.rs");

    assert!(ops_content.contains("// --- Projected operation stubs"), "operations.rs should contain operation stubs section");
    assert!(ops_content.contains("fn get_user("));
    assert!(ops_content.contains("fn hash_password(raw: String) -> String"));
    assert!(ops_content.contains("fn seed_user() -> ()"));
}

#[test]
fn rust_client_binds_the_declared_datasource_adapter() {
    let model = GenerationModel::from_vos_schema(USER_SCHEMA).unwrap();
    let files = iris_generator::emit_rust_files(&model, "mysql").unwrap();
    let (_, ops) = files.iter().find(|(name, _)| name == "operations.rs").expect("operations.rs");
    assert!(ops.contains("use iris_adapter_mysql::MysqlSource;"), "operations.rs must bind the mysql adapter crate");
    assert!(ops.contains("use super::errors::Result as MysqlResult;"), "operations.rs must alias the bound result type");
    assert!(!ops.contains("bound to a MySQL adapter"), "the datasource doc line must come from the binding, not a hardcoded label");
    let (_, errors) = files.iter().find(|(name, _)| name == "errors.rs").expect("errors.rs");
    assert!(errors.contains("pub use iris_adapter_mysql::{Error, Result};"), "errors.rs must re-export the bound adapter error surface");
}

#[test]
fn rust_client_binds_the_yydb_adapter_surface() {
    let model = GenerationModel::from_vos_schema(USER_SCHEMA).unwrap();
    let files = iris_generator::emit_rust_files(&model, "yydb").unwrap();
    let (_, ops) = files.iter().find(|(name, _)| name == "operations.rs").expect("operations.rs");
    assert!(ops.contains("use iris_connector_yydb::YydbSource;"), "operations.rs must bind the yydb connector crate");
    assert!(ops.contains("use super::errors::Result as YydbResult;"), "operations.rs must alias the bound yydb result type");
    assert!(ops.contains("&'a mut iris_connector_yydb::Connection"), "the transaction connection type must come from the binding");
    let (_, errors) = files.iter().find(|(name, _)| name == "errors.rs").expect("errors.rs");
    assert!(errors.contains("pub use iris_connector_yydb::{Error, Result};"), "errors.rs must re-export the bound yydb error surface");
}

const PAYLOAD_SCHEMA: &str = r#"
table Asset {
    @@asset_id: i64,
    label: utf8,
    body: bytes,
    embedding: vector<3>,
    doc: file,
    note: utf8?,
    maybe_body: bytes?,
    maybe_embedding: vector<4>?,
    maybe_doc: file?,
}
"#;

#[test]
fn rust_payload_fields_map_to_typed_carriers() {
    let model = GenerationModel::from_vos_schema(PAYLOAD_SCHEMA).unwrap();
    let fields = &model.tables[0].fields;
    let find = |name: &str| fields.iter().find(|f| f.name == name).unwrap_or_else(|| panic!("field `{name}`"));
    assert_eq!(find("body").rust_ty, "Vec<u8>");
    assert_eq!(find("embedding").rust_ty, "Vec<f32>");
    assert_eq!(find("embedding").vos_type, "vector<3>");
    assert_eq!(find("doc").rust_ty, "iris::FileDescriptor");
    assert_eq!(find("maybe_body").rust_ty, "Option<Vec<u8>>");
    assert_eq!(find("maybe_embedding").vos_type, "vector<4>?");
    assert_eq!(find("maybe_doc").rust_ty, "Option<iris::FileDescriptor>");
}

#[test]
fn rust_where_excludes_payload_fields_and_writes_typed_values() {
    let model = GenerationModel::from_vos_schema(PAYLOAD_SCHEMA).unwrap();
    let files = iris_generator::emit_rust_files(&model, "yydb").unwrap();
    let (_, models) = files.iter().find(|(name, _)| name == "models.rs").expect("models.rs");
    let (_, ops) = files.iter().find(|(name, _)| name == "operations.rs").expect("operations.rs");

    let where_start = models.find("pub struct AssetWhere {").expect("AssetWhere");
    let where_block = &models[where_start..where_start + models[where_start..].find("\n}").expect("where close")];
    assert!(where_block.contains("pub asset_id: Option<i64>"), "scalars keep their where slot");
    assert!(where_block.contains("pub label: Option<String>"));
    for payload in ["body", "embedding", "doc", "maybe_body", "maybe_embedding", "maybe_doc"] {
        assert!(!where_block.contains(payload), "payload `{payload}` must not join `AssetWhere`");
    }

    assert!(models.contains("__iris_value_bytes(row, \"body\")?"), "`bytes` decodes through the octets helper");
    assert!(models.contains("__iris_value_vector(row, \"embedding\")?"), "`vector` decodes through the components helper");
    assert!(models.contains("__iris_value_file(row, \"doc\")?"), "`file` decodes through the descriptor helper");
    assert!(models.contains("__iris_value_bytes(row, \"maybe_body\")"));

    assert!(ops.contains("fn to_row_write(row: &Asset) -> super::errors::Result<RowWrite>"), "payload validation needs a fallible row write");
    assert!(ops.contains("Value::Bytes(row.body.clone())"));
    assert!(ops.contains("iris::VectorValue::new(3u32, row.embedding.clone())"));
    assert!(ops.contains("Value::File(row.doc.clone())"));
    assert!(ops.contains("Value::Bytes(v.clone())"));
    assert!(ops.contains("iris::VectorValue::new(4u32, v.clone())"));
    assert!(ops.contains("Value::File(v.clone())"));
    assert!(ops.contains("Self::to_row_write(row)?"));

    for payload in ["x.body", "x.embedding", "x.doc"] {
        assert!(!ops.contains(payload), "synthesized predicates must not reference payload `{payload}`");
    }
}

#[test]
fn rust_generation_rejects_zero_dimension_vector() {
    let error = GenerationModel::from_vos_schema("table V {\n @@vid: i64,\n e: vector<0>,\n}").expect_err("zero dimension must fail");
    assert!(matches!(error, iris_generator::Error::UnsupportedType(ref detail) if detail.contains("dimension")), "{error}");
}

#[test]
fn rust_client_fails_closed_for_unconverged_datasource_adapters() {
    let model = GenerationModel::from_vos_schema(USER_SCHEMA).unwrap();
    for kind in ["sqlite", "postgres", "redis", "yyds", "reference"] {
        let error = iris_generator::emit_rust_files(&model, kind).expect_err("{kind} has no converged adapter binding");
        assert!(matches!(error, iris_generator::Error::UnsupportedDatasource(ref bound) if bound == kind), "{kind}: {error}");
        assert!(error.to_string().contains("adapter binding"), "{kind}: {error}");
    }
}

#[test]
fn rust_target_requires_a_datasource_kind() {
    let model = GenerationModel::from_vos_schema(USER_SCHEMA).unwrap();
    let error = iris_generator::generate(&model, "rust", std::path::Path::new("."), None).expect_err("rust target without a datasource must fail");
    assert!(matches!(error, iris_generator::Error::MissingDatasource), "{error}");
}
