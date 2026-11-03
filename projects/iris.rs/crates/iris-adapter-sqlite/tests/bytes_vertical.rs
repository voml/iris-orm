//! `bytes` vertical slice: BLOBs round-trip byte-for-byte through the SQLite adapter.

use core::range::Range;
use std::collections::BTreeMap;

use iris_adapter_sqlite::SqliteSource;
use iris_ir::{CmpOp, LiteralKind, PhysicalOp};
use iris_types::{FilterBinding, IrisContract, OperationBinding, Planner, RowWrite, TypedArgument, TypedColumn, TypedLogicalPlan, Value};
use vos::types::{
    CanonicalScalar, ContractArtifact, ContractFormat, ContractPayload, ContractProducer, OperationCardinality, OperationContract,
    OperationEffect, OperationId, OperationIdentityKey, OperationKind, OperationParameterContract, OperationParameterKey,
    ResolvedCanonicalType, SemanticCapability,
};

const ASSET_SCHEMA: &str = r#"
table Asset {
    @@asset_id: utf8,
    payload: bytes,
}
"#;

/// Deliberately invalid UTF-8: a BLOB must never be stringified on the way out.
fn payload(seed: u8) -> Vec<u8> {
    vec![0x00_u8, seed, 0xFF, 0xFE, b'i', b'r', b'i', b's']
}

fn open_with_assets(rows: &[(&str, Vec<u8>)]) -> SqliteSource {
    let db = SqliteSource::open_in_memory().expect("open");
    db.managed_push(ASSET_SCHEMA).expect("push schema");
    for (id, octets) in rows {
        db.insert(&RowWrite {
            table: "Asset".into(),
            primary_key: "asset_id".into(),
            fields: BTreeMap::from([("asset_id".into(), Value::Str((*id).into())), ("payload".into(), Value::Bytes(octets.clone()))]),
        })
        .expect("insert asset");
    }
    db
}

fn lookup_contract() -> IrisContract {
    let parameter = OperationParameterKey::new("value", CanonicalScalar::Bytes.as_str()).expect("parameter key");
    let key = OperationIdentityKey::new(
        &["asset".to_owned()],
        OperationKind::Query,
        "lookup_payload",
        vec![parameter],
        CanonicalScalar::Bytes.as_str(),
    )
    .expect("identity key");
    let operation = OperationContract {
        operation_id: OperationId::derive(&key),
        kind: OperationKind::Query,
        name: "lookup_payload".into(),
        canonical_path: vec!["asset".into(), "lookup_payload".into()],
        parameters: vec![OperationParameterContract {
            name: "value".into(),
            canonical_type: ResolvedCanonicalType::Scalar(CanonicalScalar::Bytes),
        }],
        result_type: Some(ResolvedCanonicalType::Scalar(CanonicalScalar::Bytes)),
        cardinality: OperationCardinality::One,
        effect: OperationEffect::Read,
        capabilities: vec![SemanticCapability::Storage],
        source_unit_id: 0,
        span: Range { start: 0, end: 1 },
        stable_id_hint: None,
    };
    let artifact = ContractArtifact::seal(
        ContractFormat::Iris,
        ContractProducer::vos(),
        ContractPayload { operations: vec![operation], services: Vec::new() },
    );
    IrisContract::from_json(&artifact.to_json().expect("serialize")).expect("load contract")
}

#[test]
fn managed_push_creates_blob_column_and_bytes_round_trip_byte_for_byte() {
    let octets = payload(1);
    let db = open_with_assets(&[("a1", octets.clone())]);

    let plan = Planner::new(SqliteSource::capabilities()).plan_source("Asset.collect()").expect("plan read");
    let rows = db.execute_plan(&plan).expect("read back");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("payload"), Some(&Value::Bytes(octets)));
}

#[test]
fn typed_contract_operation_binds_bytes_argument_and_returns_bytes_result() {
    let target = payload(2);
    let db = open_with_assets(&[("a1", target.clone()), ("a2", payload(3))]);

    let contract = lookup_contract();
    let typed = TypedLogicalPlan::bind_operation(
        &contract.operations()[0],
        contract.fingerprint(),
        OperationBinding {
            table: "Asset".into(),
            result_columns: vec![TypedColumn { name: "payload".into(), canonical_type: ResolvedCanonicalType::Scalar(CanonicalScalar::Bytes) }],
            filter: Some(FilterBinding { column: "payload".into(), op: CmpOp::Eq, parameter: "value".into() }),
        },
        &[TypedArgument { name: "value".into(), value: Value::Bytes(target.clone()) }],
    )
    .expect("bind typed plan");

    assert!(typed.physical.nodes.iter().any(|node| matches!(
        &node.op,
        PhysicalOp::Filter { predicate: iris_ir::Pred::FieldCmp { literal, kind: LiteralKind::Bytes, .. } }
            if !literal.is_empty()
    )));

    let rows = db.execute_typed_plan(&typed).expect("execute typed plan");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("payload"), Some(&Value::Bytes(target)));
}

#[test]
fn typed_bytes_result_rejects_text_stored_in_the_blob_column() {
    let db = SqliteSource::open_in_memory().expect("open");
    db.managed_push(ASSET_SCHEMA).expect("push schema");
    db.insert(&RowWrite {
        table: "Asset".into(),
        primary_key: "asset_id".into(),
        fields: BTreeMap::from([("asset_id".into(), Value::Str("a1".into())), ("payload".into(), Value::Str("not really a blob".into()))]),
    })
    .expect("insert text payload");

    let contract = lookup_contract();
    let typed = TypedLogicalPlan::bind_operation(
        &contract.operations()[0],
        contract.fingerprint(),
        OperationBinding {
            table: "Asset".into(),
            result_columns: vec![TypedColumn { name: "payload".into(), canonical_type: ResolvedCanonicalType::Scalar(CanonicalScalar::Bytes) }],
            filter: None,
        },
        &[TypedArgument { name: "value".into(), value: Value::Bytes(payload(1)) }],
    )
    .expect("bind without filter");
    let error = db.execute_typed_plan(&typed).expect_err("text cell must not decode as bytes");
    assert!(error.to_string().contains("canonical scalar `bytes`"), "error={error}");
}

#[test]
fn vector_and_file_values_keep_failing_closed_on_sqlite() {
    let db = SqliteSource::open_in_memory().expect("open");
    db.managed_push(ASSET_SCHEMA).expect("push schema");

    let vector_error = db
        .insert(&RowWrite {
            table: "Asset".into(),
            primary_key: "asset_id".into(),
            fields: BTreeMap::from([
                ("asset_id".into(), Value::Str("a1".into())),
                ("payload".into(), Value::Vector(iris_types::VectorValue::new(2, vec![1.0, 2.0]).expect("vector"))),
            ]),
        })
        .expect_err("vector has no SQLite carrier");
    assert!(vector_error.to_string().contains("yydb"), "error={vector_error}");

    let digest = "00".repeat(32);
    let descriptor = iris_types::FileDescriptor::new(digest.clone(), 4, None, None, iris_types::FileManifestRef { hash: digest, size: 4 })
        .expect("descriptor");
    let file_error = db
        .insert(&RowWrite {
            table: "Asset".into(),
            primary_key: "asset_id".into(),
            fields: BTreeMap::from([("asset_id".into(), Value::Str("a2".into())), ("payload".into(), Value::File(descriptor))]),
        })
        .expect_err("file has no SQLite carrier");
    assert!(file_error.to_string().contains("yydb"), "error={file_error}");
}
