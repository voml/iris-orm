//! Normalized Iris values / rows (Phase 1).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Phase 1 value model (VOS-aligned subset, not a SQL value).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Value {
    /// Null.
    Null,
    /// Boolean.
    Bool(bool),
    /// Integer.
    Int(i64),
    /// UTF-8 string.
    Str(String),
    /// Exact decimal value in canonical text form.
    Decimal(String),
    /// Calendar date in canonical text form.
    Date(String),
    /// Wall-clock time in canonical text form.
    Time(String),
    /// UTC datetime in canonical text form.
    DateTime(String),
    /// Octets of the VOS `bytes` scalar, exactly as the backend produced them.
    Bytes(Vec<u8>),
    /// Nested object row (query projection).
    Object(BTreeMap<String, Value>),
}

impl Value {
    /// Display / compare encoding used in predicates.
    pub fn as_pred_literal(&self) -> (String, iris_ir::LiteralKind) {
        match self {
            Self::Null => ("null".into(), iris_ir::LiteralKind::Null),
            Self::Bool(b) => (b.to_string(), iris_ir::LiteralKind::Bool),
            Self::Int(i) => (i.to_string(), iris_ir::LiteralKind::Int),
            Self::Str(s) => (s.clone(), iris_ir::LiteralKind::Str),
            Self::Decimal(s) => (s.clone(), iris_ir::LiteralKind::Decimal),
            Self::Date(s) => (s.clone(), iris_ir::LiteralKind::Date),
            Self::Time(s) => (s.clone(), iris_ir::LiteralKind::Time),
            Self::DateTime(s) => (s.clone(), iris_ir::LiteralKind::DateTime),
            Self::Bytes(octets) => (bytes_to_hex(octets), iris_ir::LiteralKind::Bytes),
            Self::Object(_) => ("null".into(), iris_ir::LiteralKind::Null),
        }
    }
}

/// Encodes octets as the lowercase hex text of the VOS `bytes` wire form.
///
/// The encoding comes from VOS itself, so a literal produced here always
/// decodes through [`hex_to_bytes`] without an intermediate text guess.
pub fn bytes_to_hex(octets: &[u8]) -> String {
    vos::types::BytesValue::new(octets.to_vec()).to_wire().data
}

/// Decodes lowercase VOS hex text back to octets.
///
/// Text outside the canonical wire form yields `None` instead of a guess;
/// every literal produced by [`Value::as_pred_literal`] is canonical.
pub fn hex_to_bytes(text: &str) -> Option<Vec<u8>> {
    let wire = vos::types::BytesValueWire {
        format_version: vos::types::BYTES_VALUE_FORMAT_VERSION.to_owned(),
        encoding: "hex".to_owned(),
        data: text.to_owned(),
    };
    vos::types::BytesValue::from_wire(wire).ok().map(vos::types::BytesValue::into_octets)
}

/// One logical row: ordered field map.
pub type Row = BTreeMap<String, Value>;
