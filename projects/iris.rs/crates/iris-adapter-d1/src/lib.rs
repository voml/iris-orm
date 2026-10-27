//! D1 target-adapter type contract for Iris.
//!
//! This crate owns the **Rust-side** D1 adapter contract: wire types,
//! [`D1Request`] / [`D1Response`], execution modes, and
//! [`D1Capabilities`].  It is intentionally **runtime-free** -- no
//! Cloudflare binding, no SQL engine, no network I/O.  WASM export and
//! TS host bridge both consume these types.
//!
//! Design source: `决策和进度表/iris/06-d1-adapter-and-edge-host.md` §3-§5.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Wire value (D1 carrier; wider than `iris_types::Value` because D1 carries
// f64 and blob natively).
// ---------------------------------------------------------------------------

/// D1 wire carrier for parameter and result values.
///
/// Superset of [`iris_types::Value`] plus f64 and blob, matching what the
/// D1 HTTP binding actually transmits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum WireValue {
    /// NULL.
    Null,
    /// Boolean.
    Bool(bool),
    /// 64-bit integer.
    Int(i64),
    /// IEEE-754 double.
    F64(f64),
    /// UTF-8 string.
    Str(String),
    /// Opaque bytes (BLOB).
    Blob(Vec<u8>),
}

impl WireValue {
    /// Convert an [`iris_types::Value`] into a [`WireValue`].
    ///
    /// D1 wire carrier for decimal/date/time/datetime is the canonical
    /// text form (same as the VOS value's string payload).  Nested objects
    /// are not valid SQL parameters and surface as NULL so callers get a
    /// clearer error later.
    pub fn from_iris(value: &iris_types::Value) -> Self {
        match value {
            iris_types::Value::Null => Self::Null,
            iris_types::Value::Bool(b) => Self::Bool(*b),
            iris_types::Value::Int(i) => Self::Int(*i),
            iris_types::Value::Str(s) => Self::Str(s.clone()),
            iris_types::Value::Decimal(s)
            | iris_types::Value::Date(s)
            | iris_types::Value::Time(s)
            | iris_types::Value::DateTime(s) => Self::Str(s.clone()),
            iris_types::Value::Object(_) => Self::Null,
        }
    }
}

// ---------------------------------------------------------------------------
// Execution mode + returning projection.
// ---------------------------------------------------------------------------

/// How D1 should execute a request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum D1ExecutionMode {
    /// Pure read; returns rows via `.all()`.
    Read,
    /// Mutation, no rows returned; uses `.run()`.
    Write,
    /// Mutation with a `RETURNING` clause; uses `.all()`.
    WriteReturning,
    /// Part of a `DB.batch([...])` call, grouped by stable `group_id`.
    Batch {
        /// Stable batch-group id (同一事务内的多个请求共享一个 id).
        group_id: String,
    },
    /// Schema-migration DDL; goes through `DB.exec()`.
    Migration,
}

/// Column set requested by `RETURNING`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReturningProjection {
    /// No `RETURNING` clause.
    #[default]
    None,
    /// `RETURNING *`.
    All,
    /// `RETURNING col1, col2, ...` with explicit column names.
    Columns(Vec<String>),
}

// ---------------------------------------------------------------------------
// Request + response.
// ---------------------------------------------------------------------------

/// A fully-lowered D1 request, ready to be handed to the TS host bridge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct D1Request {
    /// D1 SQL text (D1 方言).
    pub sql: String,
    /// Positional parameter values (wire carriers, one per `?` placeholder).
    pub params: Vec<WireValue>,
    /// Execution mode (`Read` / `Write` / `WriteReturning` / `Batch` / `Migration`).
    pub mode: D1ExecutionMode,
    /// `RETURNING` projection, if any.
    pub returning: ReturningProjection,
    /// Schema contract fingerprint for the owning operation (identifies
    /// which VOS schema this SQL was lowered from).
    pub contract_fingerprint: iris_ir::SchemaFingerprint,
}

/// Raw row returned by D1 (ordered column values by the wire carrier).
pub type D1WireRow = Vec<WireValue>;

/// D1 response wire: rows + metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct D1Response {
    /// Result rows (empty for pure-write / migration).
    pub rows: Vec<D1WireRow>,
    /// Number of rows affected by the mutation (0 for reads).
    pub row_count: u64,
    /// Last-inserted row id (if any).
    pub last_insert_rowid: Option<i64>,
}

// ---------------------------------------------------------------------------
// Operation descriptor used by capability checks.
// ---------------------------------------------------------------------------

/// Minimal descriptor of the operation the adapter is about to lower,
/// used by [`D1Capabilities::supports`] before SQL text is built.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationDescriptor {
    /// Read vs write vs schema mutation.
    pub effect: iris_ir::EffectKind,
    /// Whether this operation carries a `RETURNING` projection.
    pub needs_returning: bool,
    /// Number of SQL statements that will be batched together (1 for non-batch).
    pub batch_size: usize,
    /// Approximate SQL byte length after lowering (for size-limit checks).
    pub approx_sql_bytes: usize,
    /// Number of parameter slots after lowering.
    pub approx_params: usize,
}

// ---------------------------------------------------------------------------
// Capabilities.
// ---------------------------------------------------------------------------

/// D1 backend capability surface.
///
/// Source of truth for defaults: Cloudflare D1 public docs.  See
/// `from_docs` for the conservative default surface currently declared.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct D1Capabilities {
    /// Pure reads.
    pub read: bool,
    /// Row mutations.
    pub write: bool,
    /// `DB.batch([...])` grouping.
    pub batch: bool,
    /// `RETURNING` clause support (all three DML forms).
    pub returning: bool,
    /// DDL migration execution via `DB.exec()`.
    pub migration: bool,
    /// Whether reads inside the same session see prior writes immediately.
    pub session_reads_own_writes: bool,
    /// Hard-capped parameter count per request (Cloudflare 文档: 100).
    pub max_params: usize,
    /// Hard-capped SQL length in bytes (Cloudflare 文档: ~100 KiB).
    pub max_sql_bytes: usize,
    /// Hard-capped batch size (Cloudflare 文档: 50).
    pub max_batch_size: usize,
    /// Retryable D1 error codes (网络抖动 / 瞬时资源忙).
    pub retryable_errors: Vec<String>,
}

impl Default for D1Capabilities {
    fn default() -> Self {
        Self::from_docs()
    }
}

impl D1Capabilities {
    /// Conservative defaults from Cloudflare D1 public documentation.
    ///
    /// 参考 `决策和进度表/iris/06-d1-adapter-and-edge-host.md` §6.
    pub fn from_docs() -> Self {
        Self {
            read: true,
            write: true,
            batch: true,
            returning: true,
            migration: true,
            session_reads_own_writes: false,
            max_params: 100,
            max_sql_bytes: 100 * 1024,
            max_batch_size: 50,
            retryable_errors: vec![
                "D1_NETWORK_ERROR".into(),
                "D1_BUSY".into(),
                "D1_RESOURCE_EXHAUSTED".into(),
            ],
        }
    }

    /// Everything-on mock for unit tests.
    pub fn mock() -> Self {
        Self {
            read: true,
            write: true,
            batch: true,
            returning: true,
            migration: true,
            session_reads_own_writes: true,
            max_params: 4096,
            max_sql_bytes: 1024 * 1024,
            max_batch_size: 4096,
            retryable_errors: vec!["D1_NETWORK_ERROR".into()],
        }
    }

    /// Check whether the described operation fits this capability surface.
    ///
    /// Returns structured diagnostics (never silently downgrades) so the
    /// caller can surface `IRIS-D1-CAPABILITY-MISSING` to the user.
    pub fn supports(&self, op: &OperationDescriptor) -> Vec<iris_types::Diagnostic> {
        let mut diags = Vec::new();

        match op.effect {
            iris_ir::EffectKind::Read if !self.read => {
                diags.push(Self::diag_missing("read", op));
            }
            iris_ir::EffectKind::Write if !self.write => {
                diags.push(Self::diag_missing("write", op));
            }
            iris_ir::EffectKind::Schema if !self.migration => {
                diags.push(Self::diag_missing("migration", op));
            }
            _ => {}
        }

        if op.needs_returning && !self.returning {
            diags.push(Self::diag_missing("returning", op));
        }

        if op.batch_size > 1 && !self.batch {
            diags.push(Self::diag_missing("batch", op));
        }

        if op.approx_params > self.max_params {
            diags.push(iris_types::Diagnostic {
                code: "IRIS-D1-PARAMS-EXCEEDED".into(),
                message: format!(
                    "D1 parameter count {} exceeds max_params {}",
                    op.approx_params, self.max_params
                ),
                span: Default::default(),
                stage: iris_types::StageKind::Plan,
                backend: Some("iris-adapter-d1".into()),
                hint: Some("split the operation or reduce column/parameter count".into()),
            });
        }

        if op.approx_sql_bytes > self.max_sql_bytes {
            diags.push(iris_types::Diagnostic {
                code: "IRIS-D1-SQL-TOO-LARGE".into(),
                message: format!(
                    "D1 SQL size {} bytes exceeds max_sql_bytes {}",
                    op.approx_sql_bytes, self.max_sql_bytes
                ),
                span: Default::default(),
                stage: iris_types::StageKind::Plan,
                backend: Some("iris-adapter-d1".into()),
                hint: Some("split the operation or reduce payload size".into()),
            });
        }

        if op.batch_size > self.max_batch_size {
            diags.push(iris_types::Diagnostic {
                code: "IRIS-D1-BATCH-EXCEEDED".into(),
                message: format!(
                    "D1 batch size {} exceeds max_batch_size {}",
                    op.batch_size, self.max_batch_size
                ),
                span: Default::default(),
                stage: iris_types::StageKind::Plan,
                backend: Some("iris-adapter-d1".into()),
                hint: Some("split the batch across multiple DB.batch calls".into()),
            });
        }

        diags
    }

    fn diag_missing(what: &str, op: &OperationDescriptor) -> iris_types::Diagnostic {
        let _ = op;
        iris_types::Diagnostic {
            code: "IRIS-D1-CAPABILITY-MISSING".into(),
            message: format!("D1 capability missing: {what}"),
            span: Default::default(),
            stage: iris_types::StageKind::Plan,
            backend: Some("iris-adapter-d1".into()),
            hint: Some(format!(
                "the D1 lowering for this operation requires `{what}`; \
                 consider a different storage target or disable this feature"
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Error + result alias.
// ---------------------------------------------------------------------------

/// D1 adapter-level error (before host binding).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum D1Error {
    /// Capability check failed (full diagnostics in the diagnostic list).
    CapabilityMissing(Vec<iris_types::Diagnostic>),
    /// Fingerprint mismatch -- VOS contract identity changed underneath us.
    FingerprintMismatch {
        /// Expected contract fingerprint.
        expected: iris_ir::SchemaFingerprint,
        /// Observed contract fingerprint at lowering time.
        observed: iris_ir::SchemaFingerprint,
    },
    /// SQL text failed adapter-side preflight before being handed to host.
    SqlPreflight(String),
}

impl std::fmt::Display for D1Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CapabilityMissing(diags) => {
                write!(f, "D1 capability missing: ")?;
                let mut first = true;
                for d in diags {
                    if !first {
                        write!(f, "; ")?;
                    }
                    write!(f, "{}", d.message)?;
                    first = false;
                }
                Ok(())
            }
            Self::FingerprintMismatch { expected, observed } => write!(
                f,
                "D1 contract fingerprint mismatch: expected {}, observed {}",
                expected.0, observed.0
            ),
            Self::SqlPreflight(m) => write!(f, "D1 SQL preflight: {m}"),
        }
    }
}

impl std::error::Error for D1Error {}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, D1Error>;

// ---------------------------------------------------------------------------
// Crate metadata.
// ---------------------------------------------------------------------------

/// Adapter identifier (matches the `StorageProfile::d1` name).
pub const BACKEND_ID: &str = "d1";

/// Adapter crate version label.
pub const ADAPTER_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_value_from_iris_round_trip() {
        assert_eq!(
            WireValue::from_iris(&iris_types::Value::Null),
            WireValue::Null
        );
        assert_eq!(
            WireValue::from_iris(&iris_types::Value::Bool(true)),
            WireValue::Bool(true)
        );
        assert_eq!(
            WireValue::from_iris(&iris_types::Value::Int(42)),
            WireValue::Int(42)
        );
        assert_eq!(
            WireValue::from_iris(&iris_types::Value::Str("x".into())),
            WireValue::Str("x".into())
        );
    }

    #[test]
    fn capabilities_from_docs_are_realistic() {
        let c = D1Capabilities::from_docs();
        assert!(c.read && c.write && c.batch && c.returning && c.migration);
        assert_eq!(c.max_params, 100);
        assert_eq!(c.max_sql_bytes, 100 * 1024);
        assert_eq!(c.max_batch_size, 50);
        // eventual consistency = session_reads_own_writes is false by default.
        assert!(!c.session_reads_own_writes);
    }

    #[test]
    fn supports_finds_param_overflow() {
        let c = D1Capabilities::from_docs();
        let op = OperationDescriptor {
            effect: iris_ir::EffectKind::Write,
            needs_returning: false,
            batch_size: 1,
            approx_sql_bytes: 64,
            approx_params: 101, // > 100
        };
        let diags = c.supports(&op);
        assert!(diags
            .iter()
            .any(|d| d.code == "IRIS-D1-PARAMS-EXCEEDED"));
    }

    #[test]
    fn supports_finds_batch_overflow() {
        let c = D1Capabilities::from_docs();
        let op = OperationDescriptor {
            effect: iris_ir::EffectKind::Write,
            needs_returning: false,
            batch_size: 100, // > 50
            approx_sql_bytes: 64,
            approx_params: 1,
        };
        let diags = c.supports(&op);
        assert!(diags.iter().any(|d| d.code == "IRIS-D1-BATCH-EXCEEDED"));
    }

    #[test]
    fn supports_missing_capability_when_disabled() {
        let c = D1Capabilities {
            migration: false,
            ..D1Capabilities::mock()
        };
        let op = OperationDescriptor {
            effect: iris_ir::EffectKind::Schema,
            needs_returning: false,
            batch_size: 1,
            approx_sql_bytes: 64,
            approx_params: 0,
        };
        let diags = c.supports(&op);
        assert!(diags
            .iter()
            .any(|d| d.code == "IRIS-D1-CAPABILITY-MISSING"));
    }

    #[test]
    fn request_serde_round_trip() {
        let req = D1Request {
            sql: "SELECT ?".into(),
            params: vec![WireValue::Int(1)],
            mode: D1ExecutionMode::Read,
            returning: ReturningProjection::None,
            contract_fingerprint: iris_ir::SchemaFingerprint("abc123".into()),
        };
        let s = serde_json::to_string(&req).unwrap();
        let back: D1Request = serde_json::from_str(&s).unwrap();
        assert_eq!(back, req);
    }
}
