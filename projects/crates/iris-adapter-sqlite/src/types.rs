//! Typed value bridging between Iris rows and rusqlite values.

use iris_types::Value;

pub(crate) fn to_sql_value(value: &Value) -> rusqlite::types::Value {
    match value {
        Value::Null => rusqlite::types::Value::Null,
        Value::Bool(b) => rusqlite::types::Value::Integer(i64::from(*b)),
        Value::Int(i) => rusqlite::types::Value::Integer(*i),
        Value::Str(s) => rusqlite::types::Value::Text(s.clone()),
    }
}

pub(crate) fn from_sql_value(value: rusqlite::types::Value) -> Value {
    match value {
        rusqlite::types::Value::Null => Value::Null,
        rusqlite::types::Value::Integer(i) => Value::Int(i),
        rusqlite::types::Value::Real(f) => Value::Str(f.to_string()),
        rusqlite::types::Value::Text(s) => Value::Str(s),
        rusqlite::types::Value::Blob(b) => Value::Str(format!("blob:{}", b.len())),
    }
}
