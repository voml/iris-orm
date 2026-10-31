//! Typed value bridging between Iris rows and provider values.

use iris_ir::LiteralKind;
use iris_types::{Value, is_date, is_decimal, is_time, is_utc_datetime};
use vos::types::{CanonicalScalar, ResolvedCanonicalType};

use crate::{Error, Result, provider::SqliteValue};

pub(crate) fn to_sql_value(value: &Value) -> Result<SqliteValue> {
    Ok(match value {
        Value::Null => SqliteValue::Null,
        Value::Bool(b) => SqliteValue::Integer(i64::from(*b)),
        Value::Int(i) => SqliteValue::Integer(*i),
        Value::UInt(u) => {
            if let Ok(value) = i64::try_from(*u) {
                SqliteValue::Integer(value)
            }
            else {
                return Err(Error::Policy(format!("VOS `u64` value {u} is outside the SQLite INTEGER range")));
            }
        }
        Value::Float(f) => {
            if !f.is_finite() {
                return Err(Error::Policy("VOS `f64` values must be finite".to_owned()));
            }
            SqliteValue::Real(*f)
        }
        Value::Str(s) | Value::Decimal(s) | Value::Date(s) | Value::Time(s) | Value::DateTime(s) => SqliteValue::Text(s.as_bytes().to_vec()),
        Value::Bytes(octets) => SqliteValue::Blob(octets.clone()),
        Value::Vector(_) | Value::File(_) => {
            return Err(Error::Policy(
                "VOS `vector`/`file` payloads need a datasource that binds typed values (yydb); SQLite has no carrier for them".to_owned(),
            ));
        }
        Value::Object(_) => SqliteValue::Null,
    })
}

pub(crate) fn literal_to_sql_value(text: &str, kind: LiteralKind) -> Result<SqliteValue> {
    Ok(match kind {
        LiteralKind::Null => SqliteValue::Null,
        LiteralKind::Bool => SqliteValue::Integer(i64::from(text == "true")),
        LiteralKind::Int => SqliteValue::Integer(text.parse().unwrap_or(0)),
        LiteralKind::UInt => match text.parse::<u64>() {
            Ok(value) => SqliteValue::Integer(i64::try_from(value).map_err(|_| {
                Error::Policy(format!("VOS `u64` literal {value} is outside the SQLite INTEGER range"))
            })?),
            Err(_) => SqliteValue::Null,
        },
        LiteralKind::Float => match text.parse::<f64>() {
            Ok(value) if value.is_finite() => SqliteValue::Real(value),
            _ => return Err(Error::Policy(format!("`{text}` is not a finite VOS `f64` literal"))),
        },
        LiteralKind::Str | LiteralKind::Decimal | LiteralKind::Date | LiteralKind::Time | LiteralKind::DateTime => {
            SqliteValue::Text(text.as_bytes().to_vec())
        }
        LiteralKind::Bytes => SqliteValue::Blob(iris_types::hex_to_bytes(text).unwrap_or_default()),
        // Payload literals never reach a relational predicate (the typed plan
        // rejects them at plan time); they encode as SQL NULL.
        LiteralKind::Vector | LiteralKind::File => SqliteValue::Null,
    })
}

pub(crate) fn from_sql_value(value: SqliteValue) -> Result<Value> {
    Ok(match value {
        SqliteValue::Null => Value::Null,
        SqliteValue::Integer(i) => Value::Int(i),
        SqliteValue::Real(f) => {
            if !f.is_finite() {
                return Err(Error::Policy("SQLite returned a non-finite value for a VOS `f64`".to_owned()));
            }
            Value::Float(f)
        }
        SqliteValue::Text(bytes) => Value::Str(String::from_utf8_lossy(&bytes).into_owned()),
        SqliteValue::Blob(b) => Value::Bytes(b),
    })
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
        (scalar @ (CanonicalScalar::U8 | CanonicalScalar::U16 | CanonicalScalar::U32), SqliteValue::Integer(value)) => {
            let maximum = unsigned_max(scalar);
            if (0..=maximum).contains(&value) { Ok(Value::Int(value)) } else { Err(mismatch(scalar)) }
        }
        (CanonicalScalar::U64, SqliteValue::Integer(value)) if value >= 0 => Ok(Value::UInt(value as u64)),
        (scalar @ (CanonicalScalar::F32 | CanonicalScalar::F64), SqliteValue::Real(value)) if value.is_finite() => {
            if matches!(scalar, CanonicalScalar::F32) && f64::from(value as f32) != value {
                return Err(mismatch(scalar));
            }
            Ok(Value::Float(value))
        }
        (scalar @ (CanonicalScalar::F32 | CanonicalScalar::F64), SqliteValue::Integer(value)) => {
            if matches!(scalar, CanonicalScalar::F32) && f64::from(value as f32) != value as f64 {
                return Err(mismatch(scalar));
            }
            Ok(Value::Float(value as f64))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_binds_as_native_real_and_decodes_back() {
        let bound = to_sql_value(&Value::Float(-0.0)).expect("bind finite f64");
        assert!(matches!(bound, SqliteValue::Real(_)));
        let decoded = from_sql_value(bound).expect("decode real");
        match decoded {
            Value::Float(value) => assert_eq!(value.to_bits(), (-0.0_f64).to_bits()),
            other => panic!("expected `Value::Float`, got {other:?}"),
        }
    }

    #[test]
    fn u64_within_sqlite_range_keeps_its_unsigned_carrier() {
        assert!(matches!(to_sql_value(&Value::UInt(7)), Ok(SqliteValue::Integer(7))));
        match from_sql_value_typed(SqliteValue::Integer(i64::MAX), &ResolvedCanonicalType::Scalar(CanonicalScalar::U64)) {
            Ok(Value::UInt(value)) => assert_eq!(value, i64::MAX as u64),
            other => panic!("expected `Value::UInt`, got {other:?}"),
        }
    }

    #[test]
    fn values_sqlite_cannot_carry_are_rejected_instead_of_truncated() {
        assert!(to_sql_value(&Value::UInt(u64::MAX)).is_err());
        assert!(to_sql_value(&Value::Float(f64::NAN)).is_err());
        assert!(literal_to_sql_value("18446744073709551615", LiteralKind::UInt).is_err());
        assert!(literal_to_sql_value("NaN", LiteralKind::Float).is_err());
    }
}
