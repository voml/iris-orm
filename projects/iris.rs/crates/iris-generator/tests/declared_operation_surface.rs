//! Negative surface: generated delegates may only project declared VOS operations.
//!
//! Every case here is a stable `Error` classification — generation never falls
//! back to a guessed carrier (`String`), a guessed kind (`op`) or a guessed
//! filter (`StringFilter`), and never panics.

use iris_generator::{Error, GenerationModel, emit_target_files};
use serde_json::{Value, json};
use vos::{FieldIdentity, IDENTITY_MANIFEST_VERSION, IdentityManifest, TypeContractKind, TypeIdentity, parse_oak, resolve_contract};

const USER_SCHEMA: &str = r#"
table User {
    @@id: uuid,
    name: utf8,
}
"#;

fn contract_json() -> String {
    let projection = parse_oak(USER_SCHEMA).unwrap().project_schema().unwrap();
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
    resolve_contract(&projection, &manifest).unwrap().to_json().unwrap()
}

fn param(name: &str, type_expr: Value) -> Value {
    json!({ "name": name, "typeExpr": type_expr, "span": { "start": 0, "end": 1 } })
}

fn operation(kind: &str, name: &str, operation_id: &str, params: Vec<Value>, return_type: Value) -> Value {
    json!({
        "kind": kind,
        "name": name,
        "identityKey": format!("{kind}:{name}"),
        "operationId": operation_id,
        "parameters": params,
        "returnType": return_type,
        "signature": null,
        "body": null,
        "span": { "start": 0, "end": 1 }
    })
}

fn artifact(operations: Vec<Value>) -> String {
    json!({
        "envelope": {
            "stage": "operation-syntax-projection",
            "contractFormatVersion": "vos-operation-contract-v0",
            "languageVersion": "vos-language-v0",
            "canonicalizationVersion": "vos-canonical-v0",
            "schemaFingerprint": null,
            "sourceUnits": [{ "sourceUnitId": 0, "source": "" }]
        },
        "operations": operations
    })
    .to_string()
}

fn project_error(operations: Vec<Value>) -> Error {
    GenerationModel::from_artifacts(&contract_json(), &artifact(operations)).expect_err("artifact must be rejected")
}

fn project_model(operations: Vec<Value>) -> GenerationModel {
    GenerationModel::from_artifacts(&contract_json(), &artifact(operations)).expect("artifact must project")
}

fn target_file<'a>(files: &'a [(String, String)], name: &str) -> &'a str {
    files.iter().find(|(path, _)| path == name).map(|(_, content)| content.as_str()).unwrap_or_else(|| panic!("`{name}`"))
}

fn generation_model_json(kind: &str) -> Value {
    json!({
        "generator_version": "test",
        "schema_fingerprint": "0".repeat(64),
        "tables": [{
            "name": "User",
            "rust_type": "User",
            "fields": [{
                "name": "id",
                "rust_ty": "String",
                "vos_type": "uuid",
                "primary": true,
                "optional": false,
                "is_uuid": true
            }]
        }],
        "operations": [{
            "operation_id": "op_0123456789abcdef",
            "kind": kind,
            "name": "get_user",
            "params": [],
            "return_type": null
        }]
    })
}

#[test]
fn unknown_operation_kind_is_rejected_at_generation() {
    let model: GenerationModel = serde_json::from_value(generation_model_json("subscription")).expect("model decodes");
    for target in ["rust", "typescript"] {
        let error = emit_target_files(&model, target).expect_err("unsupported operation kind must fail");
        assert!(matches!(error, Error::UnsupportedOperationKind { ref kind, .. } if kind == "subscription"), "{target}: {error}");
    }
}

#[test]
fn declared_operation_kind_is_accepted() {
    let model: GenerationModel = serde_json::from_value(generation_model_json("query")).expect("model decodes");
    assert!(emit_target_files(&model, "rust").is_ok());
    assert!(emit_target_files(&model, "typescript").is_ok());
}

#[test]
fn operation_artifact_with_unknown_kind_is_rejected() {
    let error = project_error(vec![operation("subscription", "watch_users", "op_0123456789abcdef", vec![], Value::Null)]);
    assert!(matches!(error, Error::OperationArtifact(_)), "{error}");
}

#[test]
fn undeclared_operation_model_is_rejected() {
    let error =
        project_error(vec![operation("query", "get_ghost", "op_0123456789abcdef", vec![], json!({ "optional": { "named": ["Ghost"] } }))]);
    assert!(matches!(error, Error::UndeclaredOperationModel { ref ty, .. } if ty == "Ghost"), "{error}");
}

#[test]
fn bytes_operation_types_cross_as_the_vos_carrier() {
    let model = project_model(vec![operation(
        "udf",
        "store_blob",
        "op_0123456789abcdef",
        vec![param("payload", json!({ "named": ["bytes"] }))],
        json!({ "named": ["bytes"] }),
    )]);

    let rust = emit_target_files(&model, "rust").expect("rust target");
    let operations = target_file(&rust, "operations.rs");
    assert!(operations.contains("fn store_blob(payload: Vec<u8>) -> Vec<u8>"), "`bytes` must cross as raw octets, never text:\n{operations}");

    let typescript = emit_target_files(&model, "typescript").expect("typescript target");
    let operations = target_file(&typescript, "operations.ts");
    assert!(
        operations.contains("async store_blob(payload: Uint8Array): Promise<Uint8Array>"),
        "`bytes` must keep the `Uint8Array` carrier on both sides:\n{operations}"
    );
}

#[test]
fn optional_bytes_operation_type_keeps_the_declared_optionality() {
    let model = project_model(vec![operation(
        "udf",
        "store_optional_blob",
        "op_0123456789abcdef",
        vec![param("payload", json!({ "optional": { "named": ["bytes"] } }))],
        json!({ "optional": { "named": ["bytes"] } }),
    )]);

    let rust = emit_target_files(&model, "rust").expect("rust target");
    let operations = target_file(&rust, "operations.rs");
    assert!(
        operations.contains("fn store_optional_blob(payload: Option<Vec<u8>>) -> Option<Vec<u8>>"),
        "an optional `bytes` type must stay optional in Rust:\n{operations}"
    );

    let typescript = emit_target_files(&model, "typescript").expect("typescript target");
    let operations = target_file(&typescript, "operations.ts");
    assert!(
        operations.contains("async store_optional_blob(payload: Uint8Array): Promise<Uint8Array>"),
        "the `bytes` carrier is `Uint8Array` on both sides:\n{operations}"
    );
}

#[test]
fn vector_parameter_and_return_use_native_carrier() {
    // `vector` stays a typed payload end to end: the generator must emit the
    // `Vec<f32>` / `Float32Array` carriers on both sides of the declared-vos
    // delegate, never a string disguise.
    let model = project_model(vec![operation(
        "query",
        "search",
        "op_0123456789abcdef",
        vec![param("embedding", json!({ "named": ["vector"] }))],
        json!({ "named": ["vector"] }),
    )]);
    let op = model.operations.iter().find(|op| op.name == "search").expect("search op");
    assert_eq!(op.return_type.as_deref(), Some("vector"));
    assert_eq!(op.params.len(), 1);
    assert_eq!(op.params[0].vos_type, "vector");
    assert_eq!(op.params[0].ts_type, "Float32Array");

    let rust_files = emit_target_files(&model, "rust").expect("rust must generate");
    let rust_ops = target_file(&rust_files, "operations.rs");
    assert!(rust_ops.contains("embedding: Vec<f32>"), "Rust param carrier must be `Vec<f32>`:\n{rust_ops}");
    assert!(rust_ops.contains("-> Vec<f32>"), "Rust return carrier must be `Vec<f32>`:\n{rust_ops}");

    let ts_files = emit_target_files(&model, "typescript").expect("typescript must generate");
    let ts_ops = target_file(&ts_files, "operations.ts");
    assert!(ts_ops.contains("embedding: Float32Array"), "TS param carrier must be `Float32Array`:\n{ts_ops}");
    assert!(ts_ops.contains("Promise<Float32Array>"), "TS return carrier must be `Promise<Float32Array>`:\n{ts_ops}");
}

#[test]
fn dimensioned_vector_parameter_is_still_rejected() {
    let error = project_error(vec![operation(
        "query",
        "search",
        "op_0123456789abcdef",
        vec![param("embedding", json!({ "generic": { "path": ["vector"], "arguments": [{ "literal": "3" }] } }))],
        Value::Null,
    )]);
    assert!(matches!(error, Error::UnsupportedOperationType { ref ty, .. } if ty == "vector<3>"), "{error}");
}

#[test]
fn file_parameter_and_return_use_native_carrier() {
    // `file` descriptors stay typed end to end: the Rust side carries
    // `iris::FileDescriptor` and the TypeScript side carries the inline
    // descriptor shape, never a string disguise.
    let model = project_model(vec![operation(
        "query",
        "download",
        "op_0123456789abcdef",
        vec![param("photo", json!({ "named": ["file"] }))],
        json!({ "named": ["file"] }),
    )]);
    let op = model.operations.iter().find(|op| op.name == "download").expect("download op");
    assert_eq!(op.return_type.as_deref(), Some("file"));
    assert_eq!(op.params.len(), 1);
    assert_eq!(op.params[0].vos_type, "file");
    assert!(op.params[0].ts_type.contains("readonly digest: string"), "{}", op.params[0].ts_type);
    assert!(op.params[0].ts_type.contains("readonly octets: bigint"), "{}", op.params[0].ts_type);
    assert!(op.params[0].ts_type.contains("readonly manifest: { readonly hash: string; readonly size: bigint }"), "{}", op.params[0].ts_type);

    let rust_files = emit_target_files(&model, "rust").expect("rust must generate");
    let rust_ops = target_file(&rust_files, "operations.rs");
    assert!(rust_ops.contains("photo: iris::FileDescriptor"), "Rust param carrier must be `iris::FileDescriptor`:\n{rust_ops}");
    assert!(rust_ops.contains("-> iris::FileDescriptor"), "Rust return carrier must be `iris::FileDescriptor`:\n{rust_ops}");

    let ts_files = emit_target_files(&model, "typescript").expect("typescript must generate");
    let ts_ops = target_file(&ts_files, "operations.ts");
    assert!(
        ts_ops.contains("photo: { readonly digest: string; readonly octets: bigint;"),
        "TS param carrier must be the inline descriptor:\n{ts_ops}"
    );
    assert!(ts_ops.contains("Promise<{ readonly digest: string;"), "TS return carrier must be the inline descriptor:\n{ts_ops}");
}

#[test]
fn list_parameter_is_rejected() {
    let error = project_error(vec![operation(
        "query",
        "by_ids",
        "op_0123456789abcdef",
        vec![param("ids", json!({ "list": { "named": ["uuid"] } }))],
        Value::Null,
    )]);
    assert!(matches!(error, Error::UnsupportedOperationType { ref ty, .. } if ty == "[uuid]"), "{error}");
}

#[test]
fn unknown_label_parameter_is_rejected() {
    let error = project_error(vec![operation(
        "udf",
        "hash",
        "op_0123456789abcdef",
        vec![param("raw", json!({ "named": ["bogus"] }))],
        json!({ "named": ["utf8"] }),
    )]);
    assert!(matches!(error, Error::UnknownVosLabel(ref label) if label == "bogus"), "{error}");
}

#[test]
fn operation_without_identity_is_rejected() {
    let error = project_error(vec![operation("query", "get_user", "", vec![], Value::Null)]);
    assert!(matches!(error, Error::MissingOperationIdentity { .. }), "{error}");
}

#[test]
fn duplicate_operation_identity_is_rejected() {
    let error = project_error(vec![
        operation("query", "get_user", "op_0123456789abcdef", vec![], Value::Null),
        operation("query", "find_user", "op_0123456789abcdef", vec![], Value::Null),
    ]);
    assert!(matches!(error, Error::DuplicateOperationId { ref operation_id } if operation_id == "op_0123456789abcdef"), "{error}");
}

#[test]
fn label_without_filter_surface_is_rejected() {
    const UNIT_FIELD_SCHEMA: &str = r#"
table Doc {
    @@id: uuid,
    marker: unit,
}
"#;
    let model = GenerationModel::from_vos_schema(UNIT_FIELD_SCHEMA).expect("model");
    let error = emit_target_files(&model, "typescript").expect_err("a label without a filter type must fail");
    assert!(
        matches!(
            error,
            Error::UnsupportedFilterType { ref entity, ref field, ref vos_type }
                if entity == "Doc" && field == "marker" && vos_type == "unit"
        ),
        "{error}"
    );
}

#[test]
fn source_operations_carry_durable_identity() {
    const SCHEMA: &str = r#"
table User {
    @@id: uuid,
    name: utf8,
}

macro seed_user(prefix: utf8) -> unit {
    User::insert({ id: "1", name: prefix })
}
"#;
    let model = GenerationModel::from_vos_schema(SCHEMA).expect("model");
    let seed_user = model.operations.iter().find(|operation| operation.name == "seed_user").expect("seed_user operation");
    let projected = parse_oak(SCHEMA).unwrap().project_operations().unwrap();
    let declared = projected.iter().find(|operation| operation.name == "seed_user").expect("projected seed_user");
    assert_eq!(seed_user.operation_id, declared.operation_id.to_string());
    assert!(seed_user.operation_id.starts_with("op_"), "{}", seed_user.operation_id);
    assert_eq!(seed_user.operation_id.len(), 3 + 64);
}
