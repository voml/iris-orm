//! Closed physical operation enum (non-SQL).

use serde::{Deserialize, Serialize};

/// Comparison operator for filter predicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CmpOp {
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
}

/// Backend-neutral filter predicate (Phase 1 subset).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pred {
    /// `field == true/false`
    FieldBool {
        /// Field name.
        field: String,
        /// Expected boolean.
        value: bool,
    },
    /// `field cmp literal` where literal is bool/int/string/null encoding.
    FieldCmp {
        /// Field name.
        field: String,
        /// Comparison.
        op: CmpOp,
        /// Literal encoded as string (Phase 1); bools use `true`/`false`.
        literal: String,
        /// Literal kind tag.
        kind: LiteralKind,
    },
    /// Conjunction.
    And(Box<Pred>, Box<Pred>),
    /// Disjunction.
    Or(Box<Pred>, Box<Pred>),
}

/// Literal kind for [`Pred::FieldCmp`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LiteralKind {
    /// Boolean.
    Bool,
    /// Integer text.
    Int,
    /// UTF-8 string.
    Str,
    /// Null.
    Null,
}

/// One projected output field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectField {
    /// Output name.
    pub name: String,
    /// Source field when different from `name` (rename); `None` means same.
    pub from: Option<String>,
}

/// Sort key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortKey {
    /// Field name.
    pub field: String,
    /// Ascending when true.
    pub ascending: bool,
}

/// Closed physical op set for Phase 1 (+ stubs reserved by name in docs).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhysicalOp {
    /// Full table scan.
    Scan {
        /// Logical table name.
        table: String,
    },
    /// Row filter.
    Filter {
        /// Predicate.
        predicate: Pred,
    },
    /// Column projection / rename.
    Project {
        /// Output fields.
        fields: Vec<ProjectField>,
    },
    /// Stable sort.
    Sort {
        /// Keys in order.
        keys: Vec<SortKey>,
    },
    /// Skip first n rows.
    Skip {
        /// Count.
        count: u64,
    },
    /// Take first n rows.
    Take {
        /// Count.
        count: u64,
    },
    /// Materialize result set (execution boundary).
    Collect,
}
