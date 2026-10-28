//! Row/value → JSON wire encoding shared by the Node and browser hosts.
//!
//! JavaScript hosts cannot hold a 64-bit integer exactly in a JSON number, so
//! `i64` values outside `Number.MAX_SAFE_INTEGER` leave as decimal strings.
//! The generated `IRIS_VALUE_CARRIERS` table turns that string back into a
//! `bigint`, which means the wire never rounds a value through `f64`.
//!
//! `bytes` values leave as base64 text — the one codec the generated runtime
//! decodes back into `Uint8Array` — so octets round trip without an
//! intermediate text guess.

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

/// Encode one value: large `i64` becomes an exact decimal string, everything else keeps its JSON shape.
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
        Value::Bytes(octets) => JsonValue::String(base64_encode(octets)),
        Value::Object(fields) => {
            let mut obj = Map::new();
            for (key, value) in fields {
                obj.insert(key.clone(), value_to_json(value));
            }
            JsonValue::Object(obj)
        }
    }
}

/// Standard base64 text of octets, matching the generated runtime codec.
fn base64_encode(octets: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(octets.len().div_ceil(3) * 4);
    for chunk in octets.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied();
        let third = chunk.get(2).copied();
        out.push(ALPHABET[usize::from(first >> 2)] as char);
        out.push(ALPHABET[usize::from((first & 0x03) << 4 | second.unwrap_or(0) >> 4)] as char);
        out.push(match second {
            Some(second) => ALPHABET[usize::from((second & 0x0f) << 2 | third.unwrap_or(0) >> 6)] as char,
            None => '=',
        });
        out.push(match third {
            Some(third) => ALPHABET[usize::from(third & 0x3f)] as char,
            None => '=',
        });
    }
    out
}
