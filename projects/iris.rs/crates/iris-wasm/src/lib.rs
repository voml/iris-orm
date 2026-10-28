//! Browser WebAssembly surface for Iris ORM.
//!
//! Same semantic entry points as `iris-napi`; no parallel TS parser.

#![deny(clippy::all)]

pub mod bind;
mod core;
mod session;
pub mod wire;

pub use bind::bind_parameters;

pub use core::{
    SchemaCheck, check_artifacts, check_schema_source, generate_from_artifacts, introspect_from_artifacts, introspect_schema_json, iris_version,
};
#[allow(deprecated)]
pub use session::{MemorySession, execute_vos_memory, query_memory};

use std::collections::BTreeMap;

use wasm_bindgen::prelude::*;

use iris_adapter_d1::{D1Request, D1Response, D1WireRow, WireValue, lower_operation};

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

/// Parse and validate schema source (same semantics as `iris check`).
#[wasm_bindgen(js_name = checkSource)]
pub fn check_source(source: &str) -> CheckSourceResult {
    check_schema_source(source).into()
}

/// Read-only schema introspection JSON (`GenerationModel` shape).
#[wasm_bindgen(js_name = introspectSchema)]
pub fn introspect_schema(source: &str) -> String {
    introspect_schema_json(source)
}

/// Execute VOS DML on a fresh in-memory reference adapter; returns JSON `{ ok, rows, error }`.
#[wasm_bindgen(js_name = queryMemory)]
pub fn query_memory_js(source: &str) -> String {
    query_memory(source)
}

/// Legacy alias of [`query_memory_js`].
#[wasm_bindgen(js_name = executeVosMemory)]
pub fn execute_vos(source: &str) -> String {
    #[allow(deprecated)]
    execute_vos_memory(source)
}

/// Validate pre-resolved VOS artifacts (Route B counterpart of [`check_source`]).
#[wasm_bindgen(js_name = checkArtifacts)]
pub fn check_artifacts_js(contract_json: &str, operations_json: &str) -> CheckSourceResult {
    check_artifacts(contract_json, operations_json).into()
}

/// Read-only schema introspection JSON from pre-resolved artifacts (Route B).
#[wasm_bindgen(js_name = introspectFromArtifacts)]
pub fn introspect_from_artifacts_js(contract_json: &str, operations_json: &str) -> String {
    introspect_from_artifacts(contract_json, operations_json)
}

/// Generate bindings from pre-resolved artifacts; returns JSON
/// `{ ok, schemaFingerprint, target, files: [{ name, content }], error }`.
#[wasm_bindgen(js_name = generateFromArtifacts)]
pub fn generate_from_artifacts_js(contract_json: &str, operations_json: &str, target: &str) -> String {
    generate_from_artifacts(contract_json, operations_json, target)
}

// ---------------------------------------------------------------------------
// D1 WASM bridge exports
// ---------------------------------------------------------------------------

/// Error-free result carrier used internally by D1 WASM exports.
fn js_err<E: std::fmt::Display>(e: E) -> JsValue {
    JsValue::from_str(&e.to_string())
}

/// Cloudflare D1 `.all()` / `.run()` raw response shape.
///
/// TS host deserialises into this before handing to the WASM bridge.
/// Note: D1 `.all()` result rows come back as arrays of wire values
/// (unnamed columns) OR as named `{ column: value }` objects depending
/// on how the host bridge built the request.  The bridge emits rows as
/// raw value arrays for now and column names separately (see
/// `column_names` below).
#[derive(Debug, Clone, serde::Deserialize)]
struct D1RawResult {
    /// Rows from `.all()`.  Each row is either an array of wire carriers
    /// or a named object; the bridge emits unnamed arrays.
    #[serde(default)]
    results: Vec<Vec<WireValue>>,
    /// Whether the D1 call succeeded.
    #[serde(default = "default_true")]
    success: bool,
    /// D1 error message when `success` is false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    /// Affected row count (`.run()` returns this too).
    #[serde(default)]
    changes: u64,
    /// Last inserted row id, if any.
    #[serde(default)]
    last_row_id: Option<i64>,
    /// Optional column name vector; when present, rows are emitted as
    /// named row maps in the canonical output.
    #[serde(default)]
    column_names: Option<Vec<String>>,
}

fn default_true() -> bool {
    true
}

/// Column-name–aware canonical row.
#[derive(Debug, Clone, serde::Serialize)]
struct CanonicalRow {
    #[serde(flatten)]
    values: BTreeMap<String, iris_types::Value>,
}

/// Top-level envelope for `decode_d1_result` output.
#[derive(Debug, Clone, serde::Serialize)]
struct CanonicalResult {
    rows: Vec<CanonicalRow>,
    row_count: u64,
    last_insert_rowid: Option<i64>,
}

/// Accept a serialised [`D1Request`] and emit the prepared op envelope that
/// the TS host bridge hands to `env.DB.prepare()`.
///
/// 当输入的 `D1Request` 带有 `operation_hint` 时，这个函数会调用
/// `lower_operation` 把 canonical CRUD operation 真正 lower 成
/// D1 SQL + positional params，覆盖输入里原有的 `sql` 字段。
/// 没有 hint 时走纯 serde round-trip（向后兼容）。
///
/// 输出 envelope shape 不变:
/// ```json
/// {
///   "sql": "SELECT * FROM users WHERE id = ?",
///   "params": [{ "kind": "int", "value": 1 }, ...],
///   "mode": "read",
///   "returning": "none"
/// }
/// ```
#[wasm_bindgen(js_name = prepareD1Operation)]
pub fn prepare_d1_operation(request_json: &str) -> Result<String, JsValue> {
    prepare_d1_operation_inner(request_json).map_err(js_err)
}

/// Native-testable core of [`prepare_d1_operation`].
pub(crate) fn prepare_d1_operation_inner(request_json: &str) -> Result<String, String> {
    let mut req: D1Request = serde_json::from_str(request_json).map_err(|e| e.to_string())?;

    // 如果有 lowering hint，调用 lower_operation 覆盖 sql/params/mode/returning。
    if let Some(hint) = req.operation_hint.as_ref() {
        let prepared = lower_operation(&hint.meta, req.params.clone()).map_err(|e| e.to_string())?;
        req.sql = prepared.sql;
        req.params = prepared.params;
        req.mode = prepared.mode;
        req.returning = prepared.returning;
        // lowering 完成后清空 hint，避免泄漏到 TS host bridge。
        req.operation_hint = None;
    }

    serde_json::to_string(&req).map_err(|e| e.to_string())
}

/// Accept a Cloudflare D1 raw `.all()` / `.run()` response JSON and
/// emit the canonical value envelope that the Iris runtime consumes.
///
/// Input shape (matches D1 HTTP binding + optional column names):
/// ```json
/// {
///   "success": true,
///   "results": [[{ "kind": "int", "value": 1 }, { "kind": "str", "value": "alice" }]],
///   "changes": 1,
///   "last_row_id": 42,
///   "column_names": ["id", "name"]
/// }
/// ```
///
/// Output shape (canonical value rows):
/// ```json
/// {
///   "rows": [{ "id": { "Int": 1 }, "name": { "Str": "alice" } }],
///   "row_count": 1,
///   "last_insert_rowid": 42
/// }
/// ```
///
/// When `column_names` is absent rows fall back to positional keys
/// (`col_0`, `col_1`, ...) so the output is always shape-valid.
#[wasm_bindgen(js_name = decodeD1Result)]
pub fn decode_d1_result(raw_json: &str) -> Result<String, JsValue> {
    decode_d1_result_inner(raw_json).map_err(js_err)
}

/// Native-testable core of [`decode_d1_result`].
pub(crate) fn decode_d1_result_inner(raw_json: &str) -> Result<String, String> {
    let raw: D1RawResult = serde_json::from_str(raw_json).map_err(|e| format!("D1 decode parse failed: {e}"))?;

    if !raw.success {
        let msg = raw.error.unwrap_or_else(|| "D1 call failed".into());
        return Err(format!("D1 decode failed: {msg}"));
    }

    let D1Response { rows, row_count, last_insert_rowid } =
        D1Response { rows: raw.results, row_count: raw.changes, last_insert_rowid: raw.last_row_id };

    let col_names = raw.column_names.unwrap_or_default();
    let canonical_rows = canonicalise_rows(&rows, &col_names);

    let envelope = CanonicalResult { rows: canonical_rows, row_count, last_insert_rowid };

    serde_json::to_string(&envelope).map_err(|e| e.to_string())
}

fn canonicalise_rows(rows: &[D1WireRow], col_names: &[String]) -> Vec<CanonicalRow> {
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let mut map = BTreeMap::new();
        for (idx, wire) in row.iter().enumerate() {
            let key = col_names.get(idx).cloned().unwrap_or_else(|| format!("col_{idx}"));
            map.insert(key, wire.to_iris());
        }
        out.push(CanonicalRow { values: map });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- prepare_d1_operation tests ----------------------------------------

    #[test]
    fn prepare_d1_operation_round_trips_request() {
        let req_json = r#"{
            "sql": "SELECT * FROM users WHERE id = ?",
            "params": [{"kind": "int", "value": 42}],
            "mode": "read",
            "returning": "none",
            "contract_fingerprint": "abc123"
        }"#;
        let out = prepare_d1_operation_inner(req_json).expect("ok");
        let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid json");
        assert_eq!(parsed["sql"], "SELECT * FROM users WHERE id = ?");
        assert_eq!(parsed["mode"], "read");
        // params 数组里 kind 正确
        let params = parsed["params"].as_array().unwrap();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0]["kind"], "int");
        assert_eq!(params[0]["value"], 42);
    }

    #[test]
    fn prepare_d1_operation_rejects_garbage_json() {
        let err = prepare_d1_operation_inner("not json at all").expect_err("error");
        assert!(!err.is_empty(), "error message should not be empty");
    }

    #[test]
    fn prepare_d1_operation_accepts_write_mode() {
        let req_json = r#"{
            "sql": "INSERT INTO users (name) VALUES (?)",
            "params": [{"kind": "str", "value": "alice"}],
            "mode": "write-returning",
            "returning": "all",
            "contract_fingerprint": "deadbeef"
        }"#;
        let out = prepare_d1_operation_inner(req_json).expect("ok");
        let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid json");
        assert_eq!(parsed["mode"], "write-returning");
        assert_eq!(parsed["returning"], "all");
    }

    // -- lowering 触发测试（operation_hint 存在时真正 lower SQL）------------

    #[test]
    fn prepare_with_hint_lowers_find_overrides_placeholder_sql() {
        // 输入 sql 是占位符，operation_hint 存在时会被真正的 lowering 覆盖。
        let req_json = r#"{
            "sql": "PLACEHOLDER_SQL",
            "params": [{"kind": "int", "value": 42}],
            "mode": "write",
            "returning": {"columns": ["id", "name"]},
            "contract_fingerprint": "abc",
            "operation_hint": {
                "meta": {
                    "table": "users",
                    "key_column": "id",
                    "kind": "find",
                    "columns": ["name", "email"]
                }
            }
        }"#;
        let out = prepare_d1_operation_inner(req_json).expect("ok");
        let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid json");
        // SQL 被真正 lower 了
        assert_eq!(parsed["sql"], "SELECT * FROM \"users\" WHERE \"id\" = ?");
        // mode 从 hint 推断是 read
        assert_eq!(parsed["mode"], "read");
        // returning 也被覆盖为 none（find 不需要）
        assert_eq!(parsed["returning"], "none");
        // operation_hint 不应出现在输出里（skip_serializing_if=None）
        assert!(parsed.get("operation_hint").is_none());
    }

    #[test]
    fn prepare_with_hint_lowers_create_to_insert_returning() {
        let req_json = r#"{
            "sql": "PLACEHOLDER",
            "params": [{"kind": "str", "value": "alice"}, {"kind": "str", "value": "a@x.com"}],
            "mode": "read",
            "returning": "none",
            "contract_fingerprint": "xyz",
            "operation_hint": {
                "meta": {
                    "table": "users",
                    "key_column": "id",
                    "kind": "create",
                    "columns": ["name", "email"]
                }
            }
        }"#;
        let out = prepare_d1_operation_inner(req_json).expect("ok");
        let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid json");
        assert_eq!(parsed["sql"], "INSERT INTO \"users\" (\"name\", \"email\") VALUES (?, ?) RETURNING *");
        assert_eq!(parsed["mode"], "write-returning");
        assert_eq!(parsed["returning"], "all");
    }

    #[test]
    fn prepare_with_hint_lowers_delete_to_plain_write() {
        let req_json = r#"{
            "sql": "PLACEHOLDER",
            "params": [{"kind": "int", "value": 7}],
            "mode": "read",
            "returning": "all",
            "contract_fingerprint": "del",
            "operation_hint": {
                "meta": {
                    "table": "users",
                    "key_column": "id",
                    "kind": "delete",
                    "columns": ["name"]
                }
            }
        }"#;
        let out = prepare_d1_operation_inner(req_json).expect("ok");
        let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid json");
        assert_eq!(parsed["sql"], "DELETE FROM \"users\" WHERE \"id\" = ?");
        assert_eq!(parsed["mode"], "write");
        assert_eq!(parsed["returning"], "none");
    }

    #[test]
    fn prepare_with_hint_errors_on_param_count_mismatch() {
        let req_json = r#"{
            "sql": "PLACEHOLDER",
            "params": [{"kind": "int", "value": 1}, {"kind": "int", "value": 2}],
            "mode": "read",
            "returning": "none",
            "contract_fingerprint": "err",
            "operation_hint": {
                "meta": {
                    "table": "users",
                    "key_column": "id",
                    "kind": "find",
                    "columns": []
                }
            }
        }"#;
        let err = prepare_d1_operation_inner(req_json).expect_err("lowering should fail");
        assert!(err.contains("ParamCountMismatch") || err.contains("expected 1"));
    }

    #[test]
    fn prepare_without_hint_stays_round_trip() {
        // 没有 operation_hint 时，保持原有的 round-trip 行为
        let req_json = r#"{
            "sql": "SELECT 1",
            "params": [],
            "mode": "read",
            "returning": "none",
            "contract_fingerprint": "plain"
        }"#;
        let out = prepare_d1_operation_inner(req_json).expect("ok");
        let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid json");
        assert_eq!(parsed["sql"], "SELECT 1");
        assert_eq!(parsed["mode"], "read");
    }

    // -- decode_d1_result tests -------------------------------------------

    #[test]
    fn decode_d1_result_read_rows_with_column_names() {
        let raw = r#"{
            "success": true,
            "results": [
                [{"kind": "int", "value": 1}, {"kind": "str", "value": "alice"}],
                [{"kind": "int", "value": 2}, {"kind": "str", "value": "bob"}]
            ],
            "changes": 0,
            "last_row_id": null,
            "column_names": ["id", "name"]
        }"#;
        let out = decode_d1_result_inner(raw).expect("ok");
        let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid json");
        let rows = parsed["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        // 第一行 id → Int(1), name → Str("alice")
        let row0_id = rows[0]["id"].as_object().unwrap();
        assert!(row0_id.contains_key("Int"));
        assert_eq!(row0_id["Int"], 1);
        let row0_name = rows[0]["name"].as_object().unwrap();
        assert!(row0_name.contains_key("Str"));
        assert_eq!(row0_name["Str"], "alice");
        assert_eq!(parsed["row_count"], 0);
    }

    #[test]
    fn decode_d1_result_write_returns_changes_and_rowid() {
        let raw = r#"{
            "success": true,
            "results": [],
            "changes": 3,
            "last_row_id": 99,
            "column_names": []
        }"#;
        let out = decode_d1_result_inner(raw).expect("ok");
        let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid json");
        assert_eq!(parsed["row_count"], 3);
        assert_eq!(parsed["last_insert_rowid"], 99);
        assert!(parsed["rows"].as_array().unwrap().is_empty());
    }

    #[test]
    fn decode_d1_result_falls_back_to_positional_keys_without_column_names() {
        let raw = r#"{
            "success": true,
            "results": [
                [{"kind": "bool", "value": true}, {"kind": "null"}]
            ],
            "changes": 1,
            "last_row_id": null
        }"#;
        let out = decode_d1_result_inner(raw).expect("ok");
        let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid json");
        let row = &parsed["rows"][0];
        assert!(row.as_object().unwrap().contains_key("col_0"));
        assert!(row.as_object().unwrap().contains_key("col_1"));
    }

    #[test]
    fn decode_d1_result_surfaces_d1_error() {
        let raw = r#"{
            "success": false,
            "error": "D1_ERROR: no such table",
            "results": [],
            "changes": 0
        }"#;
        let err = decode_d1_result_inner(raw).expect_err("error");
        assert!(err.contains("D1 decode failed"), "{err}");
        assert!(err.contains("no such table"), "{err}");
    }

    #[test]
    fn wire_value_f64_integral_round_trips_as_int() {
        let wire = WireValue::F64(42.0);
        let iris = wire.to_iris();
        assert_eq!(iris, iris_types::Value::Int(42));
    }

    #[test]
    fn wire_value_f64_fractional_becomes_str() {
        let wire = WireValue::F64(3.14);
        let iris = wire.to_iris();
        assert!(matches!(iris, iris_types::Value::Str(_)));
    }
}
