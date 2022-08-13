//! The one VOS→host carrier table used by every generated surface.
//!
//! Carriers follow the VOS value mapping: `utf8`/`utf16`, `i64`/`i32` and
//! `date`/`time`/`datetimeUtc` keep distinct VOS labels even when the
//! TypeScript carrier is the same JS type. Nothing else in the generator may
//! restate this mapping.

use vos::ast::BuiltinType;

/// Strips a trailing `?` and a leading `&` to reach the base VOS type label.
pub(crate) fn base_vos_type(vos_type: &str) -> &str {
    vos_type.trim_end_matches('?').trim_start_matches('&')
}

/// TypeScript carrier of a faithful VOS label (`None` = not a mapped label).
pub(crate) fn ts_carrier_for_label(label: &str) -> Option<&'static str> {
    Some(match label {
        "bool" => "boolean",
        "i8" | "i16" | "i32" | "u8" | "u16" | "u32" | "f32" | "f64" => "number",
        "i64" | "u64" => "bigint",
        "utf8" | "utf16" | "uuid" | "decimal" | "date" | "time" | "datetimeUtc" => "string",
        "bytes" => "Uint8Array",
        "unit" => "void",
        _ => return None,
    })
}

/// Rust carrier of a faithful VOS label (`None` = not a mapped label).
pub(crate) fn rust_carrier_for_label(label: &str) -> Option<&'static str> {
    Some(match label {
        "bool" => "bool",
        "i8" => "i8",
        "i16" => "i16",
        "i32" => "i32",
        "i64" => "i64",
        "u8" => "u8",
        "u16" => "u16",
        "u32" => "u32",
        "u64" => "u64",
        "f32" => "f32",
        "f64" => "f64",
        "utf8" | "utf16" | "uuid" | "decimal" | "date" | "time" | "datetimeUtc" => "String",
        "bytes" => "Vec<u8>",
        _ => return None,
    })
}

/// Faithful VOS label of a parsed builtin (`None` = builtin this crate cannot map).
pub(crate) fn builtin_label(builtin: BuiltinType) -> Option<&'static str> {
    Some(match builtin {
        BuiltinType::Bool => "bool",
        BuiltinType::I8 => "i8",
        BuiltinType::I16 => "i16",
        BuiltinType::I32 => "i32",
        BuiltinType::I64 => "i64",
        BuiltinType::U8 => "u8",
        BuiltinType::U16 => "u16",
        BuiltinType::U32 => "u32",
        BuiltinType::U64 => "u64",
        BuiltinType::F32 => "f32",
        BuiltinType::F64 => "f64",
        BuiltinType::Utf8 => "utf8",
        BuiltinType::Utf16 => "utf16",
        BuiltinType::Uuid => "uuid",
        BuiltinType::Decimal => "decimal",
        BuiltinType::Date => "date",
        BuiltinType::Time => "time",
        BuiltinType::DateTimeUtc => "datetimeUtc",
        BuiltinType::Bytes => "bytes",
        _ => return None,
    })
}

/// Faithful VOS label of a builtin spelling.
///
/// Accepts the aliases used by the canonical resolved contract, which spells
/// `DateTime<UTC>` as `datetime` and `decimal` as `d128`, so the source path
/// and the resolved-artifact path agree on one label set.
pub(crate) fn faithful_label(spelling: &str) -> Option<&str> {
    let label = match spelling {
        "d128" => "decimal",
        "datetime" => "datetimeUtc",
        other => other,
    };
    if ts_carrier_for_label(label).is_some() { Some(label) } else { None }
}

/// TypeScript carrier of a VOS type label, wrappers stripped.
///
/// Named types and `::` paths pass through unchanged; genuinely unknown labels
/// become `unknown` instead of failing generation.
pub(crate) fn vos_type_to_ts(vos_type: &str) -> String {
    let base = base_vos_type(vos_type);
    if let Some(carrier) = ts_carrier_for_label(base) {
        return carrier.to_owned();
    }
    if base.contains("::") || base.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
        return base.to_owned();
    }
    "unknown".to_owned()
}

/// Rust carrier and faithful VOS label of a parsed builtin.
pub(crate) fn builtin_rust_and_label(builtin: BuiltinType) -> Option<(&'static str, &'static str)> {
    let label = builtin_label(builtin)?;
    let rust = rust_carrier_for_label(label)?;
    Some((rust, label))
}
