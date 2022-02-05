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

/// Parse and validate schema source (same semantics as `iris-tools check`).
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

/// Read-only schema introspection JSON (`GenerationModel` shape).
pub fn introspect_schema_json(source: &str) -> String {
    match GenerationModel::from_vos_schema(source) {
        Ok(model) => {
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
                    })
                }).collect::<Vec<Value>>(),
                "error": Value::Null,
            });
            value.to_string()
        }
        Err(err) => {
            serde_json::json!({
                "ok": false,
                "generatorVersion": "",
                "schemaFingerprint": "",
                "tables": [],
                "error": err.to_string(),
            })
            .to_string()
        }
    }
}
