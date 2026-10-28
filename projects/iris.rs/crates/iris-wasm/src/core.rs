//! Host-testable WASM/N-API shared surface (no wasm-bindgen).

use iris_generator::GenerationModel;
use serde_json::Value;

/// Outcome of validating a VOS / `.iris` schema source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaCheck {
    pub ok: bool,
    pub table_count: u32,
    pub schema_fingerprint: String,
    pub generator_version: String,
    pub error: Option<String>,
}

/// Crate version (matches `iris::version()` / workspace semver).
pub fn iris_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Parse and validate schema source (same semantics as `iris check`).
pub fn check_schema_source(source: &str) -> SchemaCheck {
    match GenerationModel::from_vos_schema(source) {
        Ok(model) => SchemaCheck {
            ok: true,
            table_count: u32::try_from(model.tables.len()).unwrap_or(u32::MAX),
            schema_fingerprint: model.schema_fingerprint,
            generator_version: model.generator_version,
            error: None,
        },
        Err(err) => SchemaCheck {
            ok: false,
            table_count: 0,
            schema_fingerprint: String::new(),
            generator_version: String::new(),
            error: Some(err.to_string()),
        },
    }
}

/// Validate pre-resolved VOS artifacts (Route B counterpart of
/// [`check_schema_source`]).
pub fn check_artifacts(contract_json: &str, operations_json: &str) -> SchemaCheck {
    match GenerationModel::from_artifacts(contract_json, operations_json) {
        Ok(model) => SchemaCheck {
            ok: true,
            table_count: u32::try_from(model.tables.len()).unwrap_or(u32::MAX),
            schema_fingerprint: model.schema_fingerprint,
            generator_version: model.generator_version,
            error: None,
        },
        Err(err) => SchemaCheck {
            ok: false,
            table_count: 0,
            schema_fingerprint: String::new(),
            generator_version: String::new(),
            error: Some(err.to_string()),
        },
    }
}

/// Read-only schema introspection JSON (`GenerationModel` shape).
pub fn introspect_schema_json(source: &str) -> String {
    match GenerationModel::from_vos_schema(source) {
        Ok(model) => model_to_introspect_json(&model),
        Err(err) => introspect_error_json(&err.to_string()),
    }
}

/// Read-only schema introspection JSON from pre-resolved artifacts (Route B).
pub fn introspect_from_artifacts(contract_json: &str, operations_json: &str) -> String {
    match GenerationModel::from_artifacts(contract_json, operations_json) {
        Ok(model) => model_to_introspect_json(&model),
        Err(err) => introspect_error_json(&err.to_string()),
    }
}

/// Generate bindings from pre-resolved artifacts and return file contents as
/// JSON (WASM in-memory path — no filesystem writes).
///
/// Returns JSON `{ ok, schemaFingerprint, target, files: [{ name, content }], error }`.
pub fn generate_from_artifacts(contract_json: &str, operations_json: &str, target: &str) -> String {
    match iris_generator::emit_from_artifacts(contract_json, operations_json, target) {
        Ok((model, files)) => {
            let file_entries: Vec<Value> =
                files.into_iter().map(|(name, content)| serde_json::json!({ "name": name, "content": content })).collect();
            serde_json::json!({
                "ok": true,
                "schemaFingerprint": model.schema_fingerprint,
                "target": target,
                "files": file_entries,
                "error": Value::Null,
            })
            .to_string()
        }
        Err(err) => serde_json::json!({
            "ok": false,
            "schemaFingerprint": "",
            "target": target,
            "files": [],
            "error": err.to_string(),
        })
        .to_string(),
    }
}

fn model_to_introspect_json(model: &GenerationModel) -> String {
    let value = serde_json::json!({
        "ok": true,
        "generatorVersion": model.generator_version,
        "schemaFingerprint": model.schema_fingerprint,
        "tables": model.tables.iter().map(|table| {
            serde_json::json!({
                "name": table.name,
                "rustType": table.rust_type,
                "fields": table.fields.iter().map(|field| {
                    let mut obj = serde_json::json!({
                        "name": field.name,
                        "rustTy": field.rust_ty,
                        "vosType": field.vos_type,
                        "primary": field.primary,
                        "optional": field.optional,
                    });
                    if let Some(target) = &field.reference_target {
                        obj.as_object_mut().expect("field json object").insert(
                            "referenceTarget".into(),
                            serde_json::Value::String(target.clone()),
                        );
                    }
                    obj
                }).collect::<Vec<Value>>(),
            })
        }).collect::<Vec<Value>>(),
        "macros": model.macros.iter().map(|macro_def| {
            serde_json::json!({
                "name": macro_def.name,
                "returnType": macro_def.return_type,
                "params": macro_def.params.iter().map(|param| {
                    serde_json::json!({
                        "name": param.name,
                        "vosType": param.vos_type,
                        "tsType": param.ts_type,
                    })
                }).collect::<Vec<Value>>(),
            })
        }).collect::<Vec<Value>>(),
        "operations": model.operations.iter().map(|op| {
            let params_json: Vec<Value> = op.params.iter().map(|p| {
                serde_json::json!({ "name": p.name, "vosType": p.vos_type, "tsType": p.ts_type })
            }).collect();
            let ret = op.return_type.as_deref();
            serde_json::json!({
                "operationId": op.operation_id,
                "kind": op.kind,
                "name": op.name,
                "identityKey": op.identity_key,
                "params": params_json,
                "returnType": ret,
            })
        }).collect::<Vec<Value>>(),
        "error": Value::Null,
    });
    value.to_string()
}

fn introspect_error_json(err: &str) -> String {
    serde_json::json!({
        "ok": false,
        "generatorVersion": "",
        "schemaFingerprint": "",
        "tables": [],
        "macros": [],
        "operations": [],
        "error": err,
    })
    .to_string()
}
