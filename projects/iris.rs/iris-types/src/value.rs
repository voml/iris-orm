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
}

impl Value {
    /// Display / compare encoding used in predicates.
    pub fn as_pred_literal(&self) -> (String, iris_ir::LiteralKind) {
        match self {
            Self::Null => ("null".into(), iris_ir::LiteralKind::Null),
            Self::Bool(b) => (b.to_string(), iris_ir::LiteralKind::Bool),
            Self::Int(i) => (i.to_string(), iris_ir::LiteralKind::Int),
            Self::Str(s) => (s.clone(), iris_ir::LiteralKind::Str),
        }
    }
}

/// One logical row: ordered field map.
pub type Row = BTreeMap<String, Value>;
