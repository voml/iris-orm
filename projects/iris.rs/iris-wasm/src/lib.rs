//! Browser WebAssembly surface for Iris ORM.
//!
//! Same semantic entry points as `iris-napi`; no parallel TS parser.

#![deny(clippy::all)]

use iris_generator::GenerationModel;
use wasm_bindgen::prelude::*;

/// Library version (matches `iris::version()` / Cargo package version).
#[wasm_bindgen(js_name = irisVersion)]
pub fn iris_version() -> String {
    iris::version().to_string()
}

/// Result of validating a VOS / `.iris` schema source via the Rust core.
#[wasm_bindgen]
pub struct CheckSourceResult {
    ok: bool,
    table_count: u32,
    schema_fingerprint: String,
    generator_version: String,
    error: Option<String>,
}

#[wasm_bindgen]
impl CheckSourceResult {
    #[wasm_bindgen(getter)]
    pub fn ok(&self) -> bool {
        self.ok
    }

    #[wasm_bindgen(getter, js_name = tableCount)]
    pub fn table_count(&self) -> u32 {
        self.table_count
    }

    #[wasm_bindgen(getter, js_name = schemaFingerprint)]
    pub fn schema_fingerprint(&self) -> String {
        self.schema_fingerprint.clone()
    }

    #[wasm_bindgen(getter, js_name = generatorVersion)]
    pub fn generator_version(&self) -> String {
        self.generator_version.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn error(&self) -> Option<String> {
        self.error.clone()
    }
}

/// Parse and validate schema source (same semantics as `iris-tools check`).
#[wasm_bindgen(js_name = checkSource)]
pub fn check_source(source: &str) -> CheckSourceResult {
    match GenerationModel::from_vos_schema(source) {
        Ok(model) => CheckSourceResult {
            ok: true,
            table_count: u32::try_from(model.tables.len()).unwrap_or(u32::MAX),
            schema_fingerprint: model.schema_fingerprint,
            generator_version: model.generator_version,
            error: None,
        },
        Err(err) => CheckSourceResult {
            ok: false,
            table_count: 0,
            schema_fingerprint: String::new(),
            generator_version: String::new(),
            error: Some(err.to_string()),
        },
    }
}
