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
            iris_types::Value::Decimal(s) | iris_types::Value::Date(s) | iris_types::Value::Time(s) | iris_types::Value::DateTime(s) => {
                Self::Str(s.clone())
            }
            iris_types::Value::Object(_) => Self::Null,
        }
    }

    /// Convert a [`WireValue`] back into the canonical [`iris_types::Value`].
    ///
    /// The reverse of [`WireValue::from_iris`]; used by `decode_d1_result`
    /// when the WASM bridge hands raw D1 rows back to the Iris runtime.
    /// `f64` rounds to `Int` when integral, otherwise surfaces as `Str`
    /// (Iris canonical value has no float carrier); Blob becomes `Null`
    /// since there is no canonical binary type in Iris `Value`.
    pub fn to_iris(&self) -> iris_types::Value {
        match self {
            Self::Null => iris_types::Value::Null,
            Self::Bool(b) => iris_types::Value::Bool(*b),
            Self::Int(i) => iris_types::Value::Int(*i),
            Self::F64(f) => {
                // Integral f64 → Int, otherwise text lossless representation.
                if f.fract() == 0.0 && f.is_finite() { iris_types::Value::Int(*f as i64) } else { iris_types::Value::Str(f.to_string()) }
            }
            Self::Str(s) => iris_types::Value::Str(s.clone()),
            Self::Blob(_) => iris_types::Value::Null,
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
// Operation lowering types.
// ---------------------------------------------------------------------------

/// Canonical CRUD operation kinds supported by the D1 lowering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationKind {
    /// SELECT by primary key (Read mode).
    Find,
    /// INSERT row(s) (WriteReturning mode when returning is requested).
    Create,
    /// UPDATE by primary key (WriteReturning mode when returning is requested).
    Update,
    /// DELETE by primary key (Write mode, no returning).
    Delete,
}

/// Operation metadata required for SQL lowering.
///
/// Phase 1: 可以 hardcode 或用最小 fixture 构造。以后接 VOS contract 时从
/// IrEnvelope / PhysicalOp 树里提取。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationMeta {
    /// Target table name (unquoted; lowering 内部做 `"table"` 引用).
    pub table: String,
    /// Primary key column name.
    pub key_column: String,
    /// Canonical operation kind.
    pub kind: OperationKind,
    /// Non-key columns touched by Create / Update (顺序必须和 params 对应).
    pub columns: Vec<String>,
}

/// Error produced during SQL lowering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    /// Unknown operation kind (custom SQL or future variant).
    UnsupportedOperation(String),
    /// Missing metadata required for this operation kind.
    MissingMetadata(String),
    /// Table / column identifier 校验失败（SQL 注入防护）.
    InvalidIdentifier(String),
    /// 参数数量与 lowering 预期不匹配.
    ParamCountMismatch {
        /// Operation kind that produced the mismatch.
        kind: OperationKind,
        /// Expected parameter count.
        expected: usize,
        /// Actual parameter count received.
        actual: usize,
    },
}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedOperation(name) => {
                write!(f, "D1 lowering: unsupported operation kind `{name}`")
            }
            Self::MissingMetadata(field) => {
                write!(f, "D1 lowering: missing metadata field `{field}`")
            }
            Self::InvalidIdentifier(name) => {
                write!(f, "D1 lowering: invalid identifier `{name}` (must match [A-Za-z_][A-Za-z0-9_]*)")
            }
            Self::ParamCountMismatch { kind, expected, actual } => {
                write!(f, "D1 lowering {:?}: expected {expected} params, got {actual}", kind)
            }
        }
    }
}

impl std::error::Error for LoweringError {}

/// Result of successful lowering: SQL text + positional wire params + mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreparedOp {
    /// D1 SQL text (already quoted identifiers, positional `?` placeholders).
    pub sql: String,
    /// Positional parameter values (one per `?`).
    pub params: Vec<WireValue>,
    /// Execution mode for the TS host bridge.
    pub mode: D1ExecutionMode,
    /// `RETURNING` projection (matches mode).
    pub returning: ReturningProjection,
}

// ---------------------------------------------------------------------------
// SQL lowering core.
// ---------------------------------------------------------------------------

/// Lower a canonical CRUD operation into D1 SQL + positional params.
///
/// # Arguments
/// * `meta` — Operation metadata (table, key column, kind, columns).
/// * `params` — Positional parameter values.  顺序约定:
///   - `Find` / `Delete`: `[key_value]`
///   - `Create`: `[col1_value, col2_value, ...]` (columns 顺序)
///   - `Update`: `[col1_value, ..., key_value]` (SET columns 顺序 + key)
///
/// # Returns
/// `PreparedOp` with D1 SQL text, wire params, D1ExecutionMode, and ReturningProjection.
pub fn lower_operation(meta: &OperationMeta, params: Vec<WireValue>) -> std::result::Result<PreparedOp, LoweringError> {
    validate_ident(&meta.table)?;
    validate_ident(&meta.key_column)?;
    for col in &meta.columns {
        validate_ident(col)?;
    }

    let table = quote_ident(&meta.table);
    let key_col = quote_ident(&meta.key_column);

    let (sql, mode, returning, expected) = match meta.kind {
        OperationKind::Find => {
            // SELECT * FROM "table" WHERE "key" = ?
            let sql = format!("SELECT * FROM {table} WHERE {key_col} = ?");
            (sql, D1ExecutionMode::Read, ReturningProjection::None, 1usize)
        }
        OperationKind::Delete => {
            // DELETE FROM "table" WHERE "key" = ?
            let sql = format!("DELETE FROM {table} WHERE {key_col} = ?");
            (sql, D1ExecutionMode::Write, ReturningProjection::None, 1usize)
        }
        OperationKind::Create => {
            // INSERT INTO "table" (col1, col2, ...) VALUES (?, ?, ...) RETURNING *
            let cols: Vec<String> = meta.columns.iter().map(|c| quote_ident(c)).collect();
            let placeholders: Vec<&str> = cols.iter().map(|_| "?").collect();
            let sql = format!("INSERT INTO {table} ({}) VALUES ({}) RETURNING *", cols.join(", "), placeholders.join(", "));
            let n = meta.columns.len();
            (sql, D1ExecutionMode::WriteReturning, ReturningProjection::All, n)
        }
        OperationKind::Update => {
            // UPDATE "table" SET col1=?, col2=?, ... WHERE "key" = ? RETURNING *
            if meta.columns.is_empty() {
                return Err(LoweringError::MissingMetadata("Update requires at least one non-key column".into()));
            }
            let sets: Vec<String> = meta.columns.iter().map(|c| format!("{} = ?", quote_ident(c))).collect();
            let sql = format!("UPDATE {table} SET {} WHERE {key_col} = ? RETURNING *", sets.join(", "));
            let n = meta.columns.len() + 1; // SET columns + key
            (sql, D1ExecutionMode::WriteReturning, ReturningProjection::All, n)
        }
    };

    if params.len() != expected {
        return Err(LoweringError::ParamCountMismatch { kind: meta.kind, expected, actual: params.len() });
    }

    Ok(PreparedOp { sql, params, mode, returning })
}

/// Validate a SQL identifier: ASCII `[A-Za-z_][A-Za-z0-9_]*` only.
///
/// 防止通过元数据字段注入恶意 SQL 片段。
fn validate_ident(name: &str) -> std::result::Result<(), LoweringError> {
    if name.is_empty() {
        return Err(LoweringError::InvalidIdentifier(name.into()));
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphabetic() && first != '_' {
        return Err(LoweringError::InvalidIdentifier(name.into()));
    }
    for c in chars {
        if !c.is_ascii_alphanumeric() && c != '_' {
            return Err(LoweringError::InvalidIdentifier(name.into()));
        }
    }
    Ok(())
}

/// Quote a SQL identifier with double quotes (D1 = SQLite dialect).
fn quote_ident(name: &str) -> String {
    format!("\"{name}\"")
}

// ---------------------------------------------------------------------------
// Request + response.
// ---------------------------------------------------------------------------

/// Optional lowering hint carried by a `D1Request`.
///
/// When present, `prepare_d1_operation_inner` 会忽略已有的 `sql` 字段，
/// 调用 [`lower_operation`] 重新生成 SQL。没有 hint 时走原有的 serde
/// round-trip 路径（向后兼容）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperationHint {
    /// Operation metadata (table, key, kind, columns).
    pub meta: OperationMeta,
}

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
    /// Optional lowering hint.  当存在时，`prepare_d1_operation` 会调用
    /// [`lower_operation`] 重新生成 SQL + params（覆盖已有值）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_hint: Option<OperationHint>,
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
            retryable_errors: vec!["D1_NETWORK_ERROR".into(), "D1_BUSY".into(), "D1_RESOURCE_EXHAUSTED".into()],
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
                message: format!("D1 parameter count {} exceeds max_params {}", op.approx_params, self.max_params),
                span: Default::default(),
                stage: iris_types::StageKind::Plan,
                backend: Some("iris-adapter-d1".into()),
                hint: Some("split the operation or reduce column/parameter count".into()),
            });
        }

        if op.approx_sql_bytes > self.max_sql_bytes {
            diags.push(iris_types::Diagnostic {
                code: "IRIS-D1-SQL-TOO-LARGE".into(),
                message: format!("D1 SQL size {} bytes exceeds max_sql_bytes {}", op.approx_sql_bytes, self.max_sql_bytes),
                span: Default::default(),
                stage: iris_types::StageKind::Plan,
                backend: Some("iris-adapter-d1".into()),
                hint: Some("split the operation or reduce payload size".into()),
            });
        }

        if op.batch_size > self.max_batch_size {
            diags.push(iris_types::Diagnostic {
                code: "IRIS-D1-BATCH-EXCEEDED".into(),
                message: format!("D1 batch size {} exceeds max_batch_size {}", op.batch_size, self.max_batch_size),
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
            Self::FingerprintMismatch { expected, observed } => {
                write!(f, "D1 contract fingerprint mismatch: expected {}, observed {}", expected.0, observed.0)
            }
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
        assert_eq!(WireValue::from_iris(&iris_types::Value::Null), WireValue::Null);
        assert_eq!(WireValue::from_iris(&iris_types::Value::Bool(true)), WireValue::Bool(true));
        assert_eq!(WireValue::from_iris(&iris_types::Value::Int(42)), WireValue::Int(42));
        assert_eq!(WireValue::from_iris(&iris_types::Value::Str("x".into())), WireValue::Str("x".into()));
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
        assert!(diags.iter().any(|d| d.code == "IRIS-D1-PARAMS-EXCEEDED"));
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
        let c = D1Capabilities { migration: false, ..D1Capabilities::mock() };
        let op = OperationDescriptor {
            effect: iris_ir::EffectKind::Schema,
            needs_returning: false,
            batch_size: 1,
            approx_sql_bytes: 64,
            approx_params: 0,
        };
        let diags = c.supports(&op);
        assert!(diags.iter().any(|d| d.code == "IRIS-D1-CAPABILITY-MISSING"));
    }

    #[test]
    fn request_serde_round_trip() {
        let req = D1Request {
            sql: "SELECT ?".into(),
            params: vec![WireValue::Int(1)],
            mode: D1ExecutionMode::Read,
            returning: ReturningProjection::None,
            contract_fingerprint: iris_ir::SchemaFingerprint("abc123".into()),
            operation_hint: None,
        };
        let s = serde_json::to_string(&req).unwrap();
        let back: D1Request = serde_json::from_str(&s).unwrap();
        assert_eq!(back, req);
        // operation_hint = None 时不应序列化到 JSON.
        assert!(!s.contains("operation_hint"));
    }

    // -- lowering 单元测试 -------------------------------------------------

    fn users_meta(kind: OperationKind) -> OperationMeta {
        OperationMeta { table: "users".into(), key_column: "id".into(), kind, columns: vec!["name".into(), "email".into()] }
    }

    #[test]
    fn lower_find_produces_select_by_key() {
        let meta = users_meta(OperationKind::Find);
        let params = vec![WireValue::Int(42)];
        let op = lower_operation(&meta, params).expect("ok");
        assert_eq!(op.sql, "SELECT * FROM \"users\" WHERE \"id\" = ?");
        assert_eq!(op.mode, D1ExecutionMode::Read);
        assert_eq!(op.returning, ReturningProjection::None);
        assert_eq!(op.params.len(), 1);
        assert_eq!(op.params[0], WireValue::Int(42));
    }

    #[test]
    fn lower_delete_produces_delete_by_key() {
        let meta = users_meta(OperationKind::Delete);
        let params = vec![WireValue::Int(7)];
        let op = lower_operation(&meta, params).expect("ok");
        assert_eq!(op.sql, "DELETE FROM \"users\" WHERE \"id\" = ?");
        assert_eq!(op.mode, D1ExecutionMode::Write);
        assert_eq!(op.returning, ReturningProjection::None);
    }

    #[test]
    fn lower_create_produces_insert_returning() {
        let meta = users_meta(OperationKind::Create);
        let params = vec![WireValue::Str("alice".into()), WireValue::Str("a@x.com".into())];
        let op = lower_operation(&meta, params).expect("ok");
        assert_eq!(op.sql, "INSERT INTO \"users\" (\"name\", \"email\") VALUES (?, ?) RETURNING *");
        assert_eq!(op.mode, D1ExecutionMode::WriteReturning);
        assert_eq!(op.returning, ReturningProjection::All);
        assert_eq!(op.params.len(), 2);
    }

    #[test]
    fn lower_update_produces_update_returning() {
        let meta = users_meta(OperationKind::Update);
        // Update params = [name, email, key]
        let params = vec![WireValue::Str("bob".into()), WireValue::Str("b@x.com".into()), WireValue::Int(99)];
        let op = lower_operation(&meta, params).expect("ok");
        assert_eq!(op.sql, "UPDATE \"users\" SET \"name\" = ?, \"email\" = ? WHERE \"id\" = ? RETURNING *");
        assert_eq!(op.mode, D1ExecutionMode::WriteReturning);
        assert_eq!(op.returning, ReturningProjection::All);
        assert_eq!(op.params.len(), 3);
        assert_eq!(op.params[2], WireValue::Int(99));
    }

    #[test]
    fn lower_rejects_invalid_table_identifier() {
        let meta =
            OperationMeta { table: "users; DROP TABLE users--".into(), key_column: "id".into(), kind: OperationKind::Find, columns: vec![] };
        let err = lower_operation(&meta, vec![WireValue::Int(1)]).expect_err("must reject");
        assert!(matches!(err, LoweringError::InvalidIdentifier(_)));
    }

    #[test]
    fn lower_rejects_empty_columns_on_update() {
        let meta = OperationMeta { table: "users".into(), key_column: "id".into(), kind: OperationKind::Update, columns: vec![] };
        let err = lower_operation(&meta, vec![WireValue::Int(1)]).expect_err("must reject");
        assert!(matches!(err, LoweringError::MissingMetadata(_)));
    }

    #[test]
    fn lower_rejects_param_count_mismatch() {
        let meta = users_meta(OperationKind::Find);
        let err = lower_operation(&meta, vec![WireValue::Int(1), WireValue::Int(2)]).expect_err("must reject");
        assert!(matches!(err, LoweringError::ParamCountMismatch { kind: OperationKind::Find, expected: 1, actual: 2 }));
    }

    #[test]
    fn lower_round_trips_serde_for_prepared_op() {
        let meta = users_meta(OperationKind::Find);
        let op = lower_operation(&meta, vec![WireValue::Int(42)]).expect("ok");
        let s = serde_json::to_string(&op).unwrap();
        let back: PreparedOp = serde_json::from_str(&s).unwrap();
        assert_eq!(back, op);
    }

    #[test]
    fn d1_request_with_hint_serde_round_trip() {
        let req = D1Request {
            sql: "PLACEHOLDER".into(),
            params: vec![WireValue::Int(42)],
            mode: D1ExecutionMode::Read,
            returning: ReturningProjection::None,
            contract_fingerprint: iris_ir::SchemaFingerprint("abc".into()),
            operation_hint: Some(OperationHint { meta: users_meta(OperationKind::Find) }),
        };
        let s = serde_json::to_string(&req).unwrap();
        assert!(s.contains("operation_hint"), "hint should serialize");
        let back: D1Request = serde_json::from_str(&s).unwrap();
        assert_eq!(back, req);
    }
}
