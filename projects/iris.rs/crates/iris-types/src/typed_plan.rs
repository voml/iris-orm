//! Typed logical plan bound to a `vos/iris.contract/v0` operation contract.
//!
//! The plan keeps the contract's parameter and result canonical types attached
//! to the physical scan/projection. Backends use those types to bind values and
//! validate result cells instead of falling back to untyped strings.

use std::collections::{BTreeMap, BTreeSet};

use iris_ir::{
    CmpOp, EffectKind, IrEnvelope, IrVersion, LiteralKind, PhysicalOp, PhysicalPlan, PlannedNode, Pred, ProjectField, RealizationClass,
    SchemaFingerprint, SemanticHash,
};
use vos::types::{CanonicalScalar, ContractFormat, OperationContract, OperationEffect, ResolvedCanonicalType, SemanticCapability};

use crate::value::Value;

/// Diagnostic code for a missing, extra or duplicated typed argument.
pub const CODE_ARGUMENT: &str = "TP010";
/// Diagnostic code for an argument value that does not match its canonical type.
pub const CODE_ARGUMENT_TYPE: &str = "TP011";
/// Diagnostic code for a parameter type the typed plan cannot lower.
pub const CODE_PARAMETER_TYPE: &str = "TP012";
/// Diagnostic code for a result contract / projected column mismatch.
pub const CODE_RESULT_TYPE: &str = "TP013";
/// Diagnostic code for a result type that cannot be projected as typed columns.
pub const CODE_RESULT_UNSUPPORTED: &str = "TP014";
/// Diagnostic code for an unusable filter binding.
pub const CODE_FILTER: &str = "TP015";
/// Diagnostic code for malformed canonical value text.
pub const CODE_CANONICAL_VALUE: &str = "TP016";

/// Structured typed-plan failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedPlanError {
    /// Stable diagnostic code.
    pub code: &'static str,
    /// Human-readable reason.
    pub message: String,
}

impl TypedPlanError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
}

impl core::fmt::Display for TypedPlanError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for TypedPlanError {}

/// One contract parameter carried by the typed plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedParameter {
    /// Contract parameter name.
    pub name: String,
    /// Contract parameter canonical type.
    pub canonical_type: ResolvedCanonicalType,
}

/// One typed result column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedColumn {
    /// Result column name.
    pub name: String,
    /// Expected canonical type of every cell in this column.
    pub canonical_type: ResolvedCanonicalType,
}

/// One argument supplied for a contract parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedArgument {
    /// Contract parameter name.
    pub name: String,
    /// Typed value. `decimal`, `date`, `time` and `datetime` use their own
    /// variants and are never supplied as untyped strings.
    pub value: Value,
}

/// Filter binding from a contract parameter to a stored column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterBinding {
    /// Stored column compared by the filter.
    pub column: String,
    /// Comparison operator.
    pub op: CmpOp,
    /// Contract parameter name whose typed value is the right-hand side.
    pub parameter: String,
}

/// Physical target a contract operation is bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationBinding {
    /// Stored table scanned by the operation.
    pub table: String,
    /// Typed result columns projected by the operation.
    pub result_columns: Vec<TypedColumn>,
    /// Optional parameter-to-column filter.
    pub filter: Option<FilterBinding>,
}

/// Typed logical plan derived from a validated operation contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedLogicalPlan {
    /// Durable operation identity.
    pub operation_id: String,
    /// Fingerprint of the consumed contract artifact.
    pub contract_fingerprint: String,
    /// Contract parameters in declaration order.
    pub parameters: Vec<TypedParameter>,
    /// Typed result columns.
    pub result_columns: Vec<TypedColumn>,
    /// Lowered physical plan consumed by backends.
    pub physical: PhysicalPlan,
}

impl TypedLogicalPlan {
    /// Binds a validated operation contract to a physical scan/projection.
    ///
    /// Every parameter must be supplied with a value matching its canonical
    /// type, and the projected result columns must agree with the declared
    /// result type. Unsupported (user-defined or generic) types are rejected
    /// instead of being flattened into text.
    pub fn bind_operation(
        operation: &OperationContract,
        contract_fingerprint: &str,
        binding: OperationBinding,
        arguments: &[TypedArgument],
    ) -> Result<Self, TypedPlanError> {
        if binding.table.trim().is_empty() {
            return Err(TypedPlanError::new(CODE_ARGUMENT, "typed plan table must not be empty"));
        }
        let mut supplied = BTreeMap::new();
        for argument in arguments {
            if supplied.insert(argument.name.as_str(), &argument.value).is_some() {
                return Err(TypedPlanError::new(CODE_ARGUMENT, format!("duplicate argument `{}`", argument.name)));
            }
        }
        let mut parameters = Vec::with_capacity(operation.parameters.len());
        for parameter in &operation.parameters {
            let value = supplied.remove(parameter.name.as_str()).ok_or_else(|| {
                TypedPlanError::new(CODE_ARGUMENT, format!("missing argument `{}` for parameter `{}`", parameter.name, parameter.name))
            })?;
            let (scalar, optional) = parameter_scalar(&parameter.canonical_type).ok_or_else(|| {
                TypedPlanError::new(CODE_PARAMETER_TYPE, format!("parameter `{}` uses a non-scalar canonical type", parameter.name))
            })?;
            require_canonical_value(&parameter.name, value, scalar, optional)?;
            parameters.push(TypedParameter { name: parameter.name.clone(), canonical_type: parameter.canonical_type.clone() });
        }
        if let Some(name) = supplied.keys().next() {
            return Err(TypedPlanError::new(CODE_ARGUMENT, format!("unknown argument `{name}`")));
        }

        validate_result_columns(operation, &binding.result_columns)?;
        let mut seen = BTreeSet::new();
        for column in &binding.result_columns {
            if column.name.trim().is_empty() {
                return Err(TypedPlanError::new(CODE_RESULT_TYPE, "typed result column names must not be empty"));
            }
            if !seen.insert(column.name.as_str()) {
                return Err(TypedPlanError::new(CODE_RESULT_TYPE, format!("duplicate typed result column `{}`", column.name)));
            }
        }

        let effect = match operation.effect {
            OperationEffect::Pure | OperationEffect::Read => EffectKind::Read,
            OperationEffect::Write => EffectKind::Write,
        };
        let mut nodes = vec![planned(PhysicalOp::Scan { table: binding.table.clone() })];
        if let Some(filter) = &binding.filter {
            let value = supplied_argument(arguments, &filter.parameter)?;
            let parameter = operation
                .parameters
                .iter()
                .find(|parameter| parameter.name == filter.parameter)
                .ok_or_else(|| TypedPlanError::new(CODE_FILTER, format!("filter parameter `{}` is not declared", filter.parameter)))?;
            let (scalar, optional) = parameter_scalar(&parameter.canonical_type)
                .ok_or_else(|| TypedPlanError::new(CODE_FILTER, format!("filter parameter `{}` is not a scalar", filter.parameter)))?;
            let (literal, kind) = canonical_literal(&parameter.name, value, scalar, optional)?;
            nodes
                .push(planned(PhysicalOp::Filter { predicate: Pred::FieldCmp { field: filter.column.clone(), op: filter.op, literal, kind } }));
        }
        if !binding.result_columns.is_empty() {
            nodes.push(planned(PhysicalOp::Project {
                fields: binding.result_columns.iter().map(|column| ProjectField { name: column.name.clone(), from: None }).collect(),
            }));
        }
        nodes.push(planned(PhysicalOp::Collect));

        let physical = PhysicalPlan {
            envelope: IrEnvelope {
                vos_contract_version: ContractFormat::VERSION_TAG.to_owned(),
                ir_version: IrVersion::PHASE1,
                schema_fingerprint: SchemaFingerprint(contract_fingerprint.to_owned()),
                operation_id: operation.operation_id.to_string(),
                effect,
                required_capabilities: operation.capabilities.iter().map(capability_name).map(str::to_owned).collect(),
                span_start: operation.span.start,
                span_end: operation.span.end,
                semantic_hash: SemanticHash(0),
            },
            nodes,
        };
        Ok(Self {
            operation_id: operation.operation_id.to_string(),
            contract_fingerprint: contract_fingerprint.to_owned(),
            parameters,
            result_columns: binding.result_columns,
            physical,
        })
    }
}

fn planned(op: PhysicalOp) -> PlannedNode {
    PlannedNode { op, realization: RealizationClass::Native, note: None }
}

fn capability_name(capability: &SemanticCapability) -> &'static str {
    match capability {
        SemanticCapability::Aggregation => "aggregation",
        SemanticCapability::Search => "search",
        SemanticCapability::Storage => "storage",
        SemanticCapability::Vector => "vector",
    }
}

fn supplied_argument<'a>(arguments: &'a [TypedArgument], name: &str) -> Result<&'a Value, TypedPlanError> {
    arguments
        .iter()
        .find(|argument| argument.name == name)
        .map(|argument| &argument.value)
        .ok_or_else(|| TypedPlanError::new(CODE_FILTER, format!("filter parameter `{name}` has no argument")))
}

fn parameter_scalar(canonical_type: &ResolvedCanonicalType) -> Option<(CanonicalScalar, bool)> {
    match canonical_type {
        ResolvedCanonicalType::Scalar(scalar) => Some((*scalar, false)),
        ResolvedCanonicalType::Optional(inner) => match inner.as_ref() {
            ResolvedCanonicalType::Scalar(scalar) => Some((*scalar, true)),
            _ => None,
        },
        _ => None,
    }
}

fn result_element_type(canonical_type: &ResolvedCanonicalType) -> Option<&ResolvedCanonicalType> {
    match canonical_type {
        ResolvedCanonicalType::List(inner) => Some(inner.as_ref()),
        _ => Some(canonical_type),
    }
}

fn validate_result_columns(operation: &OperationContract, columns: &[TypedColumn]) -> Result<(), TypedPlanError> {
    let Some(result_type) = &operation.result_type
    else {
        if columns.is_empty() {
            return Ok(());
        }
        return Err(TypedPlanError::new(CODE_RESULT_TYPE, "operation has no result but typed columns were projected"));
    };
    let expected = result_element_type(result_type)
        .ok_or_else(|| TypedPlanError::new(CODE_RESULT_UNSUPPORTED, "operation result type cannot be projected as typed columns"))?;
    if columns.len() != 1 {
        return Err(TypedPlanError::new(
            CODE_RESULT_TYPE,
            format!("operation result requires exactly one typed column, got {}", columns.len()),
        ));
    }
    let column = &columns[0];
    if &column.canonical_type != expected {
        return Err(TypedPlanError::new(CODE_RESULT_TYPE, format!("typed column `{}` does not match the operation result type", column.name)));
    }
    result_column_scalar(expected).ok_or_else(|| {
        TypedPlanError::new(CODE_RESULT_UNSUPPORTED, format!("result type of column `{}` is not a supported canonical scalar", column.name))
    })?;
    Ok(())
}

fn result_column_scalar(canonical_type: &ResolvedCanonicalType) -> Option<(CanonicalScalar, bool)> {
    match canonical_type {
        ResolvedCanonicalType::Optional(inner) => match inner.as_ref() {
            ResolvedCanonicalType::Scalar(scalar) => Some((*scalar, true)),
            _ => None,
        },
        ResolvedCanonicalType::Scalar(scalar) => Some((*scalar, false)),
        _ => None,
    }
}

fn require_canonical_value(name: &str, value: &Value, scalar: CanonicalScalar, optional: bool) -> Result<(), TypedPlanError> {
    canonical_literal(name, value, scalar, optional).map(|_| ())
}

fn canonical_literal(name: &str, value: &Value, scalar: CanonicalScalar, optional: bool) -> Result<(String, LiteralKind), TypedPlanError> {
    if matches!(value, Value::Null) {
        if optional {
            return Ok(("null".into(), LiteralKind::Null));
        }
        return Err(TypedPlanError::new(CODE_ARGUMENT_TYPE, format!("parameter `{name}` is not optional but received null")));
    }
    match (scalar, value) {
        (CanonicalScalar::Bool, Value::Bool(value)) => Ok((value.to_string(), LiteralKind::Bool)),
        (CanonicalScalar::I8, Value::Int(value)) => int_in_range(name, *value, i8::MIN as i64, i8::MAX as i64),
        (CanonicalScalar::I16, Value::Int(value)) => int_in_range(name, *value, i16::MIN as i64, i16::MAX as i64),
        (CanonicalScalar::I32, Value::Int(value)) => int_in_range(name, *value, i32::MIN as i64, i32::MAX as i64),
        (CanonicalScalar::I64, Value::Int(value)) => Ok((value.to_string(), LiteralKind::Int)),
        (CanonicalScalar::U8, Value::Int(value)) => int_in_range(name, *value, 0, u8::MAX as i64),
        (CanonicalScalar::U16, Value::Int(value)) => int_in_range(name, *value, 0, u16::MAX as i64),
        (CanonicalScalar::U32, Value::Int(value)) => int_in_range(name, *value, 0, u32::MAX as i64),
        (CanonicalScalar::U64, Value::Int(value)) if *value >= 0 => Ok((value.to_string(), LiteralKind::Int)),
        (CanonicalScalar::Utf8 | CanonicalScalar::Utf16, Value::Str(value)) => Ok((value.clone(), LiteralKind::Str)),
        (CanonicalScalar::Uuid, Value::Str(value)) if is_canonical_uuid(value) => Ok((value.clone(), LiteralKind::Str)),
        (CanonicalScalar::Uuid, Value::Str(_)) => {
            Err(TypedPlanError::new(CODE_CANONICAL_VALUE, format!("parameter `{name}` is not a canonical uuid")))
        }
        (CanonicalScalar::Decimal, Value::Decimal(value)) if is_decimal(value) => Ok((value.clone(), LiteralKind::Decimal)),
        (CanonicalScalar::Decimal, Value::Decimal(_)) => {
            Err(TypedPlanError::new(CODE_CANONICAL_VALUE, format!("parameter `{name}` is not a canonical decimal")))
        }
        (CanonicalScalar::Date, Value::Date(value)) if is_date(value) => Ok((value.clone(), LiteralKind::Date)),
        (CanonicalScalar::Date, Value::Date(_)) => {
            Err(TypedPlanError::new(CODE_CANONICAL_VALUE, format!("parameter `{name}` is not a canonical date")))
        }
        (CanonicalScalar::Time, Value::Time(value)) if is_time(value) => Ok((value.clone(), LiteralKind::Time)),
        (CanonicalScalar::Time, Value::Time(_)) => {
            Err(TypedPlanError::new(CODE_CANONICAL_VALUE, format!("parameter `{name}` is not a canonical time")))
        }
        (CanonicalScalar::DateTime, Value::DateTime(value)) if is_utc_datetime(value) => Ok((value.clone(), LiteralKind::DateTime)),
        (CanonicalScalar::DateTime, Value::DateTime(_)) => {
            Err(TypedPlanError::new(CODE_CANONICAL_VALUE, format!("parameter `{name}` is not a canonical UTC datetime")))
        }
        (CanonicalScalar::Bytes, Value::Bytes(octets)) => Ok((crate::bytes_to_hex(octets), LiteralKind::Bytes)),
        (CanonicalScalar::U64, Value::Int(_)) => {
            Err(TypedPlanError::new(CODE_ARGUMENT_TYPE, format!("parameter `{name}` is outside the u64 range representable by SQLite")))
        }
        (scalar, value) => Err(TypedPlanError::new(
            CODE_ARGUMENT_TYPE,
            format!("parameter `{name}` expects `{}` but received {}", scalar.as_str(), value_kind(value)),
        )),
    }
}

fn int_in_range(name: &str, value: i64, minimum: i64, maximum: i64) -> Result<(String, LiteralKind), TypedPlanError> {
    if (minimum..=maximum).contains(&value) {
        Ok((value.to_string(), LiteralKind::Int))
    }
    else {
        Err(TypedPlanError::new(CODE_ARGUMENT_TYPE, format!("parameter `{name}` is outside its canonical integer range")))
    }
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Int(_) => "int",
        Value::Str(_) => "text",
        Value::Decimal(_) => "decimal",
        Value::Date(_) => "date",
        Value::Time(_) => "time",
        Value::DateTime(_) => "datetime",
        Value::Bytes(_) => "bytes",
        Value::Object(_) => "object",
    }
}

/// Returns whether `text` is a canonical exact decimal.
pub fn is_decimal(text: &str) -> bool {
    let text = text.strip_prefix('-').unwrap_or(text);
    let mut parts = text.split('.');
    let (Some(integer), fraction) = (parts.next(), parts.next())
    else {
        return false;
    };
    if parts.next().is_some() || integer.is_empty() || !integer.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    fraction.is_none_or(|fraction| !fraction.is_empty() && fraction.bytes().all(|byte| byte.is_ascii_digit()))
}

/// Returns whether `text` is a canonical `YYYY-MM-DD` date.
pub fn is_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let digits = |range: core::ops::Range<usize>| text[range].bytes().all(|byte| byte.is_ascii_digit());
    if !digits(0..4) || !digits(5..7) || !digits(8..10) {
        return false;
    }
    let month: u8 = text[5..7].parse().unwrap_or(0);
    let day: u8 = text[8..10].parse().unwrap_or(0);
    (1..=12).contains(&month) && (1..=31).contains(&day)
}

/// Returns whether `text` is a canonical `HH:MM:SS` or `HH:MM:SS.fff` time.
pub fn is_time(text: &str) -> bool {
    let (clock, fraction) = text.split_once('.').map_or((text, None), |(clock, fraction)| (clock, Some(fraction)));
    let bytes = clock.as_bytes();
    if bytes.len() != 8 || bytes[2] != b':' || bytes[5] != b':' {
        return false;
    }
    if !clock[0..2].bytes().all(|byte| byte.is_ascii_digit())
        || !clock[3..5].bytes().all(|byte| byte.is_ascii_digit())
        || !clock[6..8].bytes().all(|byte| byte.is_ascii_digit())
    {
        return false;
    }
    if let Some(fraction) = fraction
        && (fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return false;
    }
    let hour: u8 = clock[0..2].parse().unwrap_or(24);
    let minute: u8 = clock[3..5].parse().unwrap_or(60);
    let second: u8 = clock[6..8].parse().unwrap_or(60);
    hour < 24 && minute < 60 && second < 60
}

/// Returns whether `text` is a canonical UTC ISO-8601 datetime ending in `Z`.
pub fn is_utc_datetime(text: &str) -> bool {
    let Some(date) = text.strip_suffix('Z')
    else {
        return false;
    };
    let Some((date, time)) = date.split_once('T')
    else {
        return false;
    };
    is_date(date) && is_time(time)
}

fn is_canonical_uuid(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != 36 || [8, 13, 18, 23].iter().any(|index| bytes[*index] != b'-') {
        return false;
    }
    bytes.iter().enumerate().all(|(index, byte)| [8, 13, 18, 23].contains(&index) || byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}
