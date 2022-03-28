//! Host-side parity tests for browser WASM check semantics.

use iris_wasm::{check_schema_source, iris_version};

const USER_SCHEMA: &str = r#"
table User {
    @@user_id: utf8,
    @user_name: utf8,
    active: bool,
}
"#;

#[test]
fn iris_version_matches_workspace_crate() {
    assert_eq!(iris_version(), env!("CARGO_PKG_VERSION"));
    assert_eq!(iris_version(), iris::version());
}

#[test]
fn check_valid_schema_matches_cli_smoke_fixture() {
    let out = check_schema_source(USER_SCHEMA);
    assert!(out.ok, "expected ok: {:?}", out.error);
    assert_eq!(out.table_count, 1);
    assert_eq!(out.schema_fingerprint, "a7ddf821fff48050");
    assert_eq!(out.generator_version, env!("CARGO_PKG_VERSION"));
    assert!(out.error.is_none());
}

#[test]
fn check_invalid_schema_returns_structured_error() {
    let out = check_schema_source("not a schema");
    assert!(!out.ok);
    assert_eq!(out.table_count, 0);
    assert!(out.schema_fingerprint.is_empty());
    assert!(out.error.as_ref().is_some_and(|e| !e.is_empty()));
}

#[test]
fn check_empty_source_yields_zero_tables() {
    let out = check_schema_source("");
    // VOS parser currently accepts empty input as an empty model (0 tables).
    assert_eq!(out.table_count, 0);
    assert!(out.schema_fingerprint.is_empty() || out.ok);
}
