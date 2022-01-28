//! Browser WebAssembly surface for Iris ORM.
//!
//! Same semantic entry points as `iris-napi`; no parallel TS parser.

#![deny(clippy::all)]

mod core;

pub use core::{SchemaCheck, check_schema_source, iris_version};

use wasm_bindgen::prelude::*;

/// Library version (matches workspace `@yydb/iris` semver).
#[wasm_bindgen(js_name = irisVersion)]
pub fn wasm_iris_version() -> String {
    iris_version()
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

impl From<SchemaCheck> for CheckSourceResult {
    fn from(value: SchemaCheck) -> Self {
        Self {
            ok: value.ok,
            table_count: value.table_count,
            schema_fingerprint: value.schema_fingerprint,
            generator_version: value.generator_version,
            error: value.error,
        }
    }
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
    check_schema_source(source).into()
}
