//! Map YYDB row/value into Iris Phase 1 value model.

use iris_types::{Row, Value};
use yydb::Value as YValue;

pub(crate) fn from_yydb_row(row: yydb::Row) -> Row {
    row.fields
        .into_iter()
        .map(|(k, v)| (k, from_yydb_value(v)))
        .collect()
}

pub(crate) fn from_yydb_value(value: YValue) -> Value {
    match value {
        YValue::Null => Value::Null,
        YValue::Bool(b) => Value::Bool(b),
        YValue::I64(i) => Value::Int(i),
        YValue::U64(u) => Value::Int(i64::try_from(u).unwrap_or(i64::MAX)),
        YValue::Text(s) => Value::Str(s),
        YValue::Uuid(u) => Value::Str(u.to_string()),
        YValue::F64(f) => Value::Str(f.to_string()),
        YValue::Bytes(b) => Value::Str(format!("bytes:{}", b.len())),
        other => Value::Str(format!("{other:?}")),
    }
}
