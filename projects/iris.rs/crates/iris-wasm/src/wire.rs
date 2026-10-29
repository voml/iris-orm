//! Row/value → JSON wire encoding shared by the Node and browser hosts.
//!
//! JavaScript hosts cannot hold a 64-bit integer exactly in a JSON number, so
//! `i64` values outside `Number.MAX_SAFE_INTEGER` leave as decimal strings.
//! The generated `IRIS_VALUE_CARRIERS` table turns that string back into a
//! `bigint`, which means the wire never rounds a value through `f64`.
//!
//! `bytes` values leave as the tagged hex carrier
//! `{"$yydb":"bytes","hex":"<lowercase hex>"}`, the shape YYDB's N-API value
//! wire uses. The generated runtime decodes the same shape back into
//! `Uint8Array`, so octets round trip without an intermediate text guess.

use iris_types::{Row, Value};
use serde_json::{Map, Value as JsonValue, json};

/// `Number.MAX_SAFE_INTEGER`: the largest integer a JSON number can hold exactly.
const JS_MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

/// Encode rows for a host JSON response, keeping `i64` exact.
pub fn rows_to_json(rows: Vec<Row>) -> Vec<JsonValue> {
    rows.into_iter()
        .map(|row| {
            let mut obj = Map::new();
            for (key, value) in row {
                obj.insert(key, value_to_json(&value));
            }
            JsonValue::Object(obj)
        })
        .collect()
}

/// Encode one value: large `i64` becomes an exact decimal string, `bytes` keeps the tagged carrier.
pub fn value_to_json(value: &Value) -> JsonValue {
    match value {
        Value::Null => JsonValue::Null,
        Value::Bool(b) => json!(b),
        Value::Int(i) => {
            if (-JS_MAX_SAFE_INTEGER..=JS_MAX_SAFE_INTEGER).contains(i) {
                json!(i)
            }
            else {
                JsonValue::String(i.to_string())
            }
        }
        Value::Str(s) | Value::Decimal(s) | Value::Date(s) | Value::Time(s) | Value::DateTime(s) => json!(s),
        Value::Bytes(octets) => iris_types::tagged_bytes_json(octets),
        Value::Object(fields) => {
            let mut obj = Map::new();
            for (key, value) in fields {
                obj.insert(key.clone(), value_to_json(value));
            }
            JsonValue::Object(obj)
        }
    }
}
