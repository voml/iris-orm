//! Host-testable WASM/N-API shared surface (no wasm-bindgen).

use iris_generator::GenerationModel;

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
