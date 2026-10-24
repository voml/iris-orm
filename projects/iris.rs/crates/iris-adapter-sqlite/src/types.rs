//! Typed value bridging between Iris rows and provider values.

use iris_ir::LiteralKind;
use iris_types::{Value, is_date, is_decimal, is_time, is_utc_datetime};
use vos::types::{CanonicalScalar, ResolvedCanonicalType};

use crate::{Error, Result, provider::SqliteValue};

pub(crate) fn to_sql_value(value: &Value) -> SqliteValue {
    match value {
        Value::Null => SqliteValue::Null,
        Value::Bool(b) => SqliteValue::Integer(i64::from(*b)),
        Value::Int(i) => SqliteValue::Integer(*i),
        Value::Str(s) | Value::Decimal(s) | Value::Date(s) | Value::Time(s) | Value::DateTime(s) => SqliteValue::Text(s.as_bytes().to_vec()),
        Value::Object(_) => SqliteValue::Null,
    }
}

pub(crate) fn literal_to_sql_value(text: &str, kind: LiteralKind) -> SqliteValue {
    match kind {
        LiteralKind::Null => SqliteValue::Null,
        LiteralKind::Bool => SqliteValue::Integer(i64::from(text == "true")),
        LiteralKind::Int => SqliteValue::Integer(text.parse().unwrap_or(0)),
        LiteralKind::Str | LiteralKind::Decimal | LiteralKind::Date | LiteralKind::Time | LiteralKind::DateTime => {
            SqliteValue::Text(text.as_bytes().to_vec())
        }
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

/// Decodes one raw cell against the contract's canonical type.
///
/// A cell whose SQLite storage class or canonical text does not match the
/// expected type is rejected instead of being stringified.
pub(crate) fn from_sql_value_typed(value: SqliteValue, canonical_type: &ResolvedCanonicalType) -> Result<Value> {
    match canonical_type {
        ResolvedCanonicalType::Optional(inner) if matches!(value, SqliteValue::Null) => Ok(Value::Null),
        ResolvedCanonicalType::Optional(inner) => from_sql_value_typed(value, inner),
        ResolvedCanonicalType::Scalar(scalar) => decode_scalar(value, *scalar),
        other => Err(Error::Policy(format!("typed result column uses unsupported canonical type `{other:?}`"))),
    }
}

fn decode_scalar(value: SqliteValue, scalar: CanonicalScalar) -> Result<Value> {
    match (scalar, value) {
        (CanonicalScalar::Bool, SqliteValue::Integer(0)) => Ok(Value::Bool(false)),
        (CanonicalScalar::Bool, SqliteValue::Integer(1)) => Ok(Value::Bool(true)),
        (scalar @ (CanonicalScalar::I8 | CanonicalScalar::I16 | CanonicalScalar::I32 | CanonicalScalar::I64), SqliteValue::Integer(value)) => {
            let (minimum, maximum) = signed_range(scalar);
            if (minimum..=maximum).contains(&value) { Ok(Value::Int(value)) } else { Err(mismatch(scalar)) }
        }
        (scalar @ (CanonicalScalar::U8 | CanonicalScalar::U16 | CanonicalScalar::U32 | CanonicalScalar::U64), SqliteValue::Integer(value)) => {
            let maximum = unsigned_max(scalar);
            if (0..=maximum).contains(&value) { Ok(Value::Int(value)) } else { Err(mismatch(scalar)) }
        }
        (CanonicalScalar::Utf8 | CanonicalScalar::Utf16, SqliteValue::Text(bytes)) => {
            Ok(Value::Str(String::from_utf8_lossy(&bytes).into_owned()))
        }
        (CanonicalScalar::Uuid, SqliteValue::Text(bytes)) => Ok(Value::Str(String::from_utf8_lossy(&bytes).into_owned())),
        (CanonicalScalar::Decimal, SqliteValue::Text(bytes)) => typed_text(scalar, &bytes, is_decimal).map(Value::Decimal),
        (CanonicalScalar::Date, SqliteValue::Text(bytes)) => typed_text(scalar, &bytes, is_date).map(Value::Date),
        (CanonicalScalar::Time, SqliteValue::Text(bytes)) => typed_text(scalar, &bytes, is_time).map(Value::Time),
        (CanonicalScalar::DateTime, SqliteValue::Text(bytes)) => typed_text(scalar, &bytes, is_utc_datetime).map(Value::DateTime),
        (scalar, _) => Err(mismatch(scalar)),
    }
}

fn typed_text(scalar: CanonicalScalar, bytes: &[u8], validate: fn(&str) -> bool) -> Result<String> {
    let text = String::from_utf8_lossy(bytes).into_owned();
    if validate(&text) { Ok(text) } else { Err(mismatch(scalar)) }
}

fn mismatch(scalar: CanonicalScalar) -> Error {
    Error::Policy(format!("typed result cell does not match canonical scalar `{}`", scalar.as_str()))
}

fn signed_range(scalar: CanonicalScalar) -> (i64, i64) {
    match scalar {
        CanonicalScalar::I8 => (i8::MIN as i64, i8::MAX as i64),
        CanonicalScalar::I16 => (i16::MIN as i64, i16::MAX as i64),
        CanonicalScalar::I32 => (i32::MIN as i64, i32::MAX as i64),
        _ => (i64::MIN, i64::MAX),
    }
}

fn unsigned_max(scalar: CanonicalScalar) -> i64 {
    match scalar {
        CanonicalScalar::U8 => u8::MAX as i64,
        CanonicalScalar::U16 => u16::MAX as i64,
        CanonicalScalar::U32 => u32::MAX as i64,
        _ => i64::MAX,
    }
}
