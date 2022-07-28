//! Typed value bridging between Iris rows and provider values.

use iris_ir::LiteralKind;
use iris_types::Value;
use sqlite_provider::SqliteValue;

pub(crate) fn to_sql_value(value: &Value) -> SqliteValue {
    match value {
        Value::Null => SqliteValue::Null,
        Value::Bool(b) => SqliteValue::Integer(i64::from(*b)),
        Value::Int(i) => SqliteValue::Integer(*i),
        Value::Str(s) => SqliteValue::Text(s.as_bytes().to_vec()),
        Value::Object(_) => SqliteValue::Null,
    }
}

pub(crate) fn literal_to_sql_value(text: &str, kind: LiteralKind) -> SqliteValue {
    match kind {
        LiteralKind::Null => SqliteValue::Null,
        LiteralKind::Bool => SqliteValue::Integer(i64::from(text == "true")),
        LiteralKind::Int => SqliteValue::Integer(text.parse().unwrap_or(0)),
        LiteralKind::Str => SqliteValue::Text(text.as_bytes().to_vec()),
    }
}

pub(crate) fn from_sql_value(value: SqliteValue) -> Value {
    match value {
        SqliteValue::Null => Value::Null,
        SqliteValue::Integer(i) => Value::Int(i),
        SqliteValue::Real(f) => Value::Str(f.to_string()),
        SqliteValue::Text(bytes) => Value::Str(String::from_utf8_lossy(&bytes).into_owned()),
        SqliteValue::Blob(b) => Value::Str(format!("blob:{}", b.len())),
    }
}
