//! Node N-API surface for Iris ORM.
//!
//! Thin binding over the Rust semantic core — no parallel TS parser.

#![deny(clippy::all)]

use iris_generator::GenerationModel;
use napi::bindgen_prelude::*;
use napi_derive::napi;

/// Library version (matches `iris::version()` / Cargo package version).
#[napi]
pub fn iris_version() -> String {
    iris::version().to_string()
}

/// Result of validating a VOS / `.iris` schema source via the Rust core.
#[napi(object)]
pub struct CheckSourceResult {
    pub ok: bool,
    pub table_count: u32,
    pub schema_fingerprint: String,
    pub generator_version: String,
    pub error: Option<String>,
}

/// Parse and validate schema source (same semantics as `iris-tools check`).
#[napi]
pub fn check_source(source: String) -> Result<CheckSourceResult> {
    match GenerationModel::from_vos_schema(&source) {
        Ok(model) => Ok(CheckSourceResult {
            ok: true,
            table_count: u32::try_from(model.tables.len()).unwrap_or(u32::MAX),
            schema_fingerprint: model.schema_fingerprint,
            generator_version: model.generator_version,
            error: None,
        }),
        Err(err) => Ok(CheckSourceResult {
            ok: false,
            table_count: 0,
            schema_fingerprint: String::new(),
            generator_version: String::new(),
            error: Some(err.to_string()),
        }),
    }
}
