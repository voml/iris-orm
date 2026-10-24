//! Structured diagnostics.

use core::range::Range;

/// Pipeline stage where a diagnostic was produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageKind {
    /// Parse.
    Parse,
    /// Semantic / lower.
    Semantic,
    /// Bind datasource.
    Bind,
    /// Plan / capability.
    Plan,
    /// Prepare.
    Prepare,
    /// Execute.
    Execute,
    /// Normalize results.
    Normalize,
}

/// Iris diagnostic (stable code + span + advice).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Stable machine code (`IRIS-PLAN-REJECTED`, ...).
    pub code: String,
    /// Human message.
    pub message: String,
    /// Source span.
    pub span: Range<usize>,
    /// Pipeline stage.
    pub stage: StageKind,
    /// Datasource / backend id when known.
    pub backend: Option<String>,
    /// Repair hint.
    pub hint: Option<String>,
}

impl Diagnostic {
    /// Build a planning rejection diagnostic.
    pub fn plan_rejected(message: impl Into<String>, span: Range<usize>, hint: Option<String>) -> Self {
        Self { code: "IRIS-PLAN-REJECTED".into(), message: message.into(), span, stage: StageKind::Plan, backend: None, hint }
    }

    /// Build a parse diagnostic from an Oak/VOS parse message.
    pub fn from_vos_parse(message: impl Into<String>) -> Self {
        Self { code: "IRIS-PARSE".into(), message: message.into(), span: (0..0).into(), stage: StageKind::Parse, backend: None, hint: None }
    }
}
