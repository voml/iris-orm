//! Contract operation -> typed logical plan -> SQLite execution.

use core::range::Range;
use std::collections::BTreeMap;

use iris_adapter_sqlite::SqliteSource;
use iris_ir::{CmpOp, LiteralKind, PhysicalOp};
use iris_types::{
    CODE_ARGUMENT_TYPE, CODE_RESULT_TYPE, FilterBinding, IrisContract, OperationBinding, RowWrite, TypedArgument, TypedColumn,
    TypedLogicalPlan, Value,
};
use vos::types::{
    CanonicalScalar, ContractArtifact, ContractFormat, ContractPayload, ContractProducer, OperationCardinality, OperationContract,
    OperationEffect, OperationId, OperationIdentityKey, OperationKind, OperationParameterContract, OperationParameterKey,
    ResolvedCanonicalType, SemanticCapability,
};

const METRIC_SCHEMA: &str = r#"
table Metric {
    @@id: utf8,
    amount: decimal,
    day: date,
    clock: time,
    instant: datetime,
}
"#;

const ID: &str = "metric-1";

fn typed_row() -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("id".into(), Value::Str(ID.into())),
        ("amount".into(), Value::Decimal("12.34".into())),
        ("day".into(), Value::Date("2026-10-25".into())),
        ("clock".into(), Value::Time("12:34:56".into())),
        ("instant".into(), Value::DateTime("2026-10-25T12:34:56Z".into())),
    ])
}

fn operation(name: &str, scalar: CanonicalScalar) -> OperationContract {
    let parameter = OperationParameterKey::new("value", scalar.as_str()).expect("parameter key");
    let key =
        OperationIdentityKey::new(&["metric".to_owned()], OperationKind::Query, name, vec![parameter], scalar.as_str()).expect("identity key");
    OperationContract {
        operation_id: OperationId::derive(&key),
        kind: OperationKind::Query,
        name: name.into(),
        canonical_path: vec!["metric".into(), name.into()],
        parameters: vec![OperationParameterContract { name: "value".into(), canonical_type: ResolvedCanonicalType::Scalar(scalar) }],
        result_type: Some(ResolvedCanonicalType::Scalar(scalar)),
        cardinality: OperationCardinality::One,
        effect: OperationEffect::Read,
        capabilities: vec![SemanticCapability::Storage],
        source_unit_id: 0,
        span: Range { start: 0, end: 1 },
    }
}

fn contract(name: &str, scalar: CanonicalScalar) -> IrisContract {
    let artifact = ContractArtifact::seal(
        ContractFormat::Iris,
        ContractProducer::vos(),
        ContractPayload { operations: vec![operation(name, scalar)], services: Vec::new() },
    );
    IrisContract::from_json(&artifact.to_json().expect("serialize")).expect("load contract")
}

fn plan(contract: &IrisContract, column: &str, scalar: CanonicalScalar, value: Value) -> TypedLogicalPlan {
    TypedLogicalPlan::bind_operation(
        &contract.operations()[0],
        contract.fingerprint(),
        OperationBinding {
            table: "Metric".into(),
            result_columns: vec![TypedColumn { name: column.into(), canonical_type: ResolvedCanonicalType::Scalar(scalar) }],
            filter: Some(FilterBinding { column: column.into(), op: CmpOp::Eq, parameter: "value".into() }),
        },
        &[TypedArgument { name: "value".into(), value }],
    )
    .expect("bind typed plan")
}

#[test]
fn contract_operation_executes_with_typed_decimal_date_time_and_datetime_results() {
    let db = SqliteSource::open_in_memory().expect("open");
    db.managed_push(METRIC_SCHEMA).expect("push schema");
    db.insert(&RowWrite { table: "Metric".into(), primary_key: "id".into(), fields: typed_row() }).expect("insert typed row");

    let cases = [
        ("lookup_amount", "amount", CanonicalScalar::Decimal, Value::Decimal("12.34".into()), LiteralKind::Decimal),
        ("lookup_day", "day", CanonicalScalar::Date, Value::Date("2026-10-25".into()), LiteralKind::Date),
        ("lookup_clock", "clock", CanonicalScalar::Time, Value::Time("12:34:56".into()), LiteralKind::Time),
        ("lookup_instant", "instant", CanonicalScalar::DateTime, Value::DateTime("2026-10-25T12:34:56Z".into()), LiteralKind::DateTime),
    ];

    for (name, column, scalar, value, kind) in cases {
        let contract = contract(name, scalar);
        let typed = plan(&contract, column, scalar, value.clone());
        assert!(typed.physical.nodes.iter().any(|node| matches!(
            &node.op,
            PhysicalOp::Filter { predicate: iris_ir::Pred::FieldCmp { literal, kind: actual, .. } }
                if actual == &kind && !literal.is_empty()
        )));
        let rows = db.execute_typed_plan(&typed).expect("execute typed plan");
        assert_eq!(rows.len(), 1, "{name}");
        assert_eq!(rows[0].get(column), Some(&value), "{name}");
    }
}

#[test]
fn rejects_untyped_string_arguments_for_typed_parameters() {
    let contract = contract("lookup_amount", CanonicalScalar::Decimal);
    let error = TypedLogicalPlan::bind_operation(
        &contract.operations()[0],
        contract.fingerprint(),
        OperationBinding {
            table: "Metric".into(),
            result_columns: vec![TypedColumn {
                name: "amount".into(),
                canonical_type: ResolvedCanonicalType::Scalar(CanonicalScalar::Decimal),
            }],
            filter: None,
        },
        &[TypedArgument { name: "value".into(), value: Value::Str("12.34".into()) }],
    )
    .expect_err("string fallback must be rejected");
    assert_eq!(error.code, CODE_ARGUMENT_TYPE);
}

#[test]
fn rejects_result_columns_that_disagree_with_the_contract_result_type() {
    let contract = contract("lookup_amount", CanonicalScalar::Decimal);
    let error = TypedLogicalPlan::bind_operation(
        &contract.operations()[0],
        contract.fingerprint(),
        OperationBinding {
            table: "Metric".into(),
            result_columns: vec![TypedColumn { name: "amount".into(), canonical_type: ResolvedCanonicalType::Scalar(CanonicalScalar::Date) }],
            filter: None,
        },
        &[TypedArgument { name: "value".into(), value: Value::Decimal("12.34".into()) }],
    )
    .expect_err("result mismatch");
    assert_eq!(error.code, CODE_RESULT_TYPE);
}

#[test]
fn rejects_malformed_canonical_text_in_typed_result_cells() {
    let db = SqliteSource::open_in_memory().expect("open");
    db.managed_push(METRIC_SCHEMA).expect("push schema");
    let mut fields = typed_row();
    fields.insert("amount".into(), Value::Str("not-a-decimal".into()));
    db.insert(&RowWrite { table: "Metric".into(), primary_key: "id".into(), fields }).expect("insert");

    let contract = contract("lookup_amount", CanonicalScalar::Decimal);
    let typed = TypedLogicalPlan::bind_operation(
        &contract.operations()[0],
        contract.fingerprint(),
        OperationBinding {
            table: "Metric".into(),
            result_columns: vec![TypedColumn {
                name: "amount".into(),
                canonical_type: ResolvedCanonicalType::Scalar(CanonicalScalar::Decimal),
            }],
            filter: None,
        },
        &[TypedArgument { name: "value".into(), value: Value::Decimal("12.34".into()) }],
    )
    .expect("bind without filter");
    let error = db.execute_typed_plan(&typed).expect_err("malformed typed cell");
    assert!(error.to_string().contains("canonical scalar `decimal`"), "error={error}");
}
