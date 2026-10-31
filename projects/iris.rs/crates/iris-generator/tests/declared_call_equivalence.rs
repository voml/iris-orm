//! Generated call and direct call must converge on one declared operation.
//!
//! The generated client is not allowed to be a second execution path: it must
//! use the same `synthesizeOperationCall` + `buildDeclaredOperationRequest`
//! helpers a direct caller imports, carry the artifact's durable `operation_id`
//! (never the author name) and pass arguments positionally in declared order.
//! With those four properties, the generated request is byte-identical to a
//! direct request for the same arguments, so both reach the same typed plan,
//! identity, diagnostics and executor-level capability gate.

use iris_generator::{GenerationModel, emit_rust_files, emit_typescript_client};
use serde_json::{Value, json};
use vos::{FieldIdentity, IDENTITY_MANIFEST_VERSION, IdentityManifest, TypeContractKind, TypeIdentity, parse_oak, resolve_contract};

const USER_SCHEMA: &str = r#"
table User {
    @@id: uuid,
    name: utf8,
}
"#;

const GET_USER: &str = "op_a1b2c3d4e5f6a7b8";
const HASH_PASSWORD: &str = "op_b1b2c3d4e5f6a7b8";
const SEED_USER: &str = "op_c1b2c3d4e5f6a7b8";
const LABEL_USER: &str = "op_d1b2c3d4e5f6a7b8";

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

fn param(name: &str, ty: &str) -> Value {
    json!({ "name": name, "typeExpr": { "named": [ty] }, "span": { "start": 0, "end": 1 } })
}

fn operation(kind: &str, name: &str, operation_id: &str, parameters: Vec<Value>, return_type: Value) -> Value {
    json!({
        "kind": kind,
        "name": name,
        "identityKey": format!("{kind}:{name}"),
        "operationId": operation_id,
        "parameters": parameters,
        "returnType": return_type,
        "signature": null,
        "body": null,
        "span": { "start": 0, "end": 1 }
    })
}

fn artifact() -> String {
    let operations = vec![
        operation("query", "get_user", GET_USER, vec![param("id", "uuid")], json!({ "optional": { "named": ["User"] } })),
        operation("udf", "hash_password", HASH_PASSWORD, vec![param("raw", "utf8")], json!({ "named": ["utf8"] })),
        operation("macro", "seed_user", SEED_USER, vec![], Value::Null),
        operation("macro", "label_user", LABEL_USER, vec![param("id", "uuid"), param("prefix", "utf8")], Value::Null),
    ];
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

fn model() -> GenerationModel {
    GenerationModel::from_artifacts(&contract_json(), &artifact()).expect("model")
}

fn typescript_file<'a>(files: &'a [(String, String)], name: &str) -> &'a str {
    files.iter().find(|(path, _)| path == name).map(|(_, content)| content.as_str()).unwrap_or_else(|| panic!("`{name}`"))
}

/// Every `operationId` literal the generated client passes to the request builder.
fn request_identity_literals(ops: &str) -> Vec<String> {
    ops.match_indices("buildDeclaredOperationRequest(\"")
        .map(|(index, marker)| {
            let start = index + marker.len();
            let tail = &ops[start..];
            tail[..tail.find('"').expect("closing quote")].to_owned()
        })
        .collect()
}

/// Identities the current table-derived CRUD delegate still invents from a table
/// name. Documented deviation in the architecture roadmap: closing it needs the
/// table CRUD surface replaced by declared read/write operations, plus the
/// examples/user-doc sync that removal implies.
const TABLE_DERIVED_PSEUDO_OPERATIONS: &[&str] =
    &["User.findMany", "User.findFirst", "User.findUnique", "User.create", "User.update", "User.delete"];

#[test]
fn generated_calls_carry_the_declared_operation_identity() {
    let files = emit_typescript_client(&model()).expect("emit");
    let ops = typescript_file(&files, "operations.ts");

    let mut identities = request_identity_literals(ops);
    identities.sort();
    identities.dedup();
    let declared = [GET_USER, HASH_PASSWORD, SEED_USER, LABEL_USER];
    for operation_id in declared {
        assert!(identities.contains(&operation_id.to_owned()), "missing declared identity `{operation_id}`:\n{ops}");
    }

    // Any identity outside the declared operations must be one of the known
    // table-derived pseudo-operations. A new name-derived entry point fails here
    // instead of silently shipping.
    let extra: Vec<&str> = identities.iter().map(String::as_str).filter(|id| !declared.contains(id)).collect();
    let mut expected: Vec<&str> = TABLE_DERIVED_PSEUDO_OPERATIONS.to_vec();
    expected.sort_unstable();
    assert_eq!(extra, expected, "undeclared identity class invented by the generator:\n{ops}");

    for name in ["get_user", "hash_password", "seed_user", "label_user"] {
        assert!(
            !ops.contains(&format!("buildDeclaredOperationRequest(\"{name}\"")),
            "generated `{name}` must not use the author name as identity:\n{ops}"
        );
    }
}

#[test]
fn generated_and_direct_calls_share_one_request_builder() {
    let files = emit_typescript_client(&model()).expect("emit");
    let ops = typescript_file(&files, "operations.ts");
    let synthesize = typescript_file(&files, "_internal/synthesize.ts");

    for helper in ["synthesizeOperationCall", "buildDeclaredOperationRequest"] {
        assert!(
            synthesize.contains(&format!("export function {helper}")),
            "`{helper}` must be the one shared export direct callers use:\n{synthesize}"
        );
        assert!(ops.contains(helper), "`operations.ts` must call the shared `{helper}`:\n{ops}");
    }

    // No second request literal, no second fingerprint, no generated-side gate.
    for forbidden in ["{ identity:", "contractFingerprint", "negotiateCapabilities", "requiredCapabilities"] {
        assert!(
            !ops.contains(forbidden),
            "`operations.ts` must not re-implement request shaping or capability negotiation (`{forbidden}`):\n{ops}"
        );
    }
}

#[test]
fn generated_calls_bind_parameters_positionally_in_declared_order() {
    let files = emit_typescript_client(&model()).expect("emit");
    let ops = typescript_file(&files, "operations.ts");

    for (call, expected) in [
        ("get_user", "synthesizeOperationCall(\"get_user\", [id])"),
        ("hash_password", "synthesizeOperationCall(\"hash_password\", [raw])"),
        ("seed_user", "synthesizeOperationCall(\"seed_user\", [])"),
        ("label_user", "synthesizeOperationCall(\"label_user\", [id, prefix])"),
    ] {
        assert!(ops.contains(expected), "generated `{call}` must bind declared parameter order `{expected}`:\n{ops}");
    }

    // Macro delegates exist twice (`$macros` and `GeneratedOperations`). Both must
    // synthesize the same call, otherwise the two delegates diverge on plan input.
    for call in ["synthesizeOperationCall(\"seed_user\", [])", "synthesizeOperationCall(\"label_user\", [id, prefix])"] {
        assert_eq!(ops.matches(call).count(), 2, "every macro delegate must share `{call}`:\n{ops}");
    }
}

#[test]
fn rust_stubs_carry_the_same_declared_operation_identity() {
    let files = emit_rust_files(&model(), "mysql").expect("emit");
    let ops = typescript_file(&files, "operations.rs");

    for (name, operation_id) in [("get_user", GET_USER), ("hash_password", HASH_PASSWORD), ("seed_user", SEED_USER), ("label_user", LABEL_USER)]
    {
        assert!(ops.contains(&format!("fn {name}(")), "missing Rust stub `{name}`:\n{ops}");
        assert!(ops.contains(&format!("(op_id: {operation_id})")), "Rust stub `{name}` must cite `{operation_id}`:\n{ops}");
    }
}
