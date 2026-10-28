//! Strict `vos/iris.contract/v0` envelope consumption.

use core::range::Range;

use iris_types::{CODE_ENVELOPE, CODE_FINGERPRINT, CODE_PRODUCER, IrisContract};
use vos::types::{
    CanonicalScalar, ContractArtifact, ContractFormat, ContractPayload, ContractProducer, OperationCardinality, OperationContract,
    OperationEffect, OperationId, OperationIdentityKey, OperationKind, OperationParameterContract, OperationParameterKey,
    ResolvedCanonicalType, SemanticCapability,
};

fn sample_operation() -> OperationContract {
    let parameter = OperationParameterKey::new("total", CanonicalScalar::Decimal.as_str()).expect("parameter key");
    let key =
        OperationIdentityKey::new(&["demo".to_owned()], OperationKind::Query, "total", vec![parameter], CanonicalScalar::Decimal.as_str())
            .expect("identity key");
    OperationContract {
        operation_id: OperationId::derive(&key),
        kind: OperationKind::Query,
        name: "total".into(),
        canonical_path: vec!["demo".into(), "total".into()],
        parameters: vec![OperationParameterContract {
            name: "total".into(),
            canonical_type: ResolvedCanonicalType::Scalar(CanonicalScalar::Decimal),
        }],
        result_type: Some(ResolvedCanonicalType::Scalar(CanonicalScalar::Decimal)),
        cardinality: OperationCardinality::One,
        effect: OperationEffect::Read,
        capabilities: vec![SemanticCapability::Storage],
        source_unit_id: 0,
        span: Range { start: 0, end: 12 },
        stable_id_hint: None,
    }
}

fn artifact(format: ContractFormat) -> ContractArtifact {
    ContractArtifact::seal(
        format,
        ContractProducer::new("vos-language", "0.0.3"),
        ContractPayload { operations: vec![sample_operation()], services: Vec::new() },
    )
}

fn tampered(artifact: &ContractArtifact, edit: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut value: serde_json::Value = serde_json::from_str(&artifact.to_json().expect("serialize")).expect("json");
    edit(&mut value);
    serde_json::to_string(&value).expect("serialize tampered")
}

#[test]
fn loads_the_iris_v0_envelope_and_exposes_contract_identities() {
    let artifact = artifact(ContractFormat::Iris);
    let json = artifact.to_json().expect("serialize");

    let contract = IrisContract::from_json(&json).expect("load");
    assert_eq!(contract.format(), ContractFormat::Iris);
    assert_eq!(contract.fingerprint(), artifact.fingerprint);
    assert_eq!(contract.producer(), &ContractProducer::new("vos-language", "0.0.3"));
    assert_eq!(contract.operations().len(), 1);
    assert!(contract.services().is_empty());
    let operation = &contract.operations()[0];
    assert!(contract.operation(&operation.operation_id).is_some());
    assert_eq!(operation.parameters[0].canonical_type, ResolvedCanonicalType::Scalar(CanonicalScalar::Decimal));
}

#[test]
fn rejects_unknown_contract_versions_with_art010() {
    let artifact = artifact(ContractFormat::Iris);
    let json = tampered(&artifact, |value| {
        value["version"] = serde_json::Value::String("vos/iris.contract/v1".into());
    });
    let error = IrisContract::from_json(&json).expect_err("unknown version");
    assert_eq!(error.code, CODE_ENVELOPE);
    assert!(error.message.contains("vos/iris.contract/v0"), "message={}", error.message);
}

#[test]
fn rejects_the_hermes_envelope_with_art010() {
    let artifact = artifact(ContractFormat::Hermes);
    let error = IrisContract::from_json(&artifact.to_json().expect("serialize")).expect_err("foreign format");
    assert_eq!(error.code, CODE_ENVELOPE);
    assert!(error.message.contains("vos/hermes.contract/v0"), "message={}", error.message);
}

#[test]
fn rejects_unknown_envelope_fields_with_art010() {
    let artifact = artifact(ContractFormat::Iris);
    let json = tampered(&artifact, |value| {
        value["unexpected"] = serde_json::Value::Bool(true);
    });
    let error = IrisContract::from_json(&json).expect_err("unknown field");
    assert_eq!(error.code, CODE_ENVELOPE);
}

#[test]
fn rejects_fingerprint_mismatch_with_art020() {
    let artifact = artifact(ContractFormat::Iris);
    let json = tampered(&artifact, |value| {
        value["fingerprint"] = serde_json::Value::String("0".repeat(64));
    });
    let error = IrisContract::from_json(&json).expect_err("fingerprint mismatch");
    assert_eq!(error.code, CODE_FINGERPRINT);
}

#[test]
fn rejects_missing_or_empty_producer_provenance_with_art021() {
    let artifact = artifact(ContractFormat::Iris);
    let missing_name = tampered(&artifact, |value| {
        value["producer"].as_object_mut().expect("producer").remove("name");
    });
    let error = IrisContract::from_json(&missing_name).expect_err("missing producer name");
    assert_eq!(error.code, CODE_PRODUCER);

    let empty_version = tampered(&artifact, |value| {
        value["producer"]["version"] = serde_json::Value::String("  ".into());
    });
    let error = IrisContract::from_json(&empty_version).expect_err("empty producer version");
    assert_eq!(error.code, CODE_PRODUCER);
}
