//! Local serde-aligned mirror of VOS `OperationProjectionArtifact`.
//!
//! Route B: Iris defines its own top-level envelope/projection structs so that
//! the generator consumes a stable JSON shape independent of VOS's internal
//! type evolution. Nested leaf types (`CanonicalType`) are reused from the
//! public `vos::types` API since those are stable VOS primitives.
//!
//! Every struct here carries `#[serde(rename_all = "camelCase")]` and
//! `#[serde(deny_unknown_fields)]` to match VOS's strict artifact contract.

use core::range::Range;

use serde::{Deserialize, Serialize};
use vos::types::{CanonicalType, ContractEnvelope, OperationKind};

/// Error type for operation artifact decoding / validation.
#[derive(Debug, Clone, thiserror::Error)]
pub enum OperationArtifactError {
    /// Serde deserialization failure.
    #[error("operation artifact decode: {0}")]
    Decode(String),
    /// Version / stage / identity invariant violation.
    #[error("operation artifact validate [{code}]: {message}")]
    Validate {
        /// Stable validation code.
        code: String,
        /// Human-readable message.
        message: String,
    },
}

/// Versioned, strict operation projection artifact that Iris can consume
/// without re-parsing VOS Oak source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct IrisOperationArtifact {
    /// Artifact envelope and source provenance (reused from VOS for
    /// compatibility across consumers).
    pub envelope: ContractEnvelope,
    /// Operation declarations in source order. Order is not an identity rule.
    pub operations: Vec<IrisOperationProjection>,
}

/// One projected operation: query, udf, micro, or macro.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct IrisOperationProjection {
    /// Operation declaration kind.
    pub kind: OperationKind,
    /// Operation author name.
    pub name: String,
    /// Opaque canonical signature key used as input to identity assignment.
    pub identity_key: String,
    /// Opaque stable operation identity derived from `identity_key`.
    pub operation_id: String,
    /// Parameters projected from the signature.
    pub parameters: Vec<IrisOperationParameter>,
    /// Return type when present.
    pub return_type: Option<CanonicalType>,
    /// Exact signature source slice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<IrisSyntaxSlice>,
    /// Exact body source slice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<IrisSyntaxSlice>,
    /// Full declaration byte span.
    #[serde(serialize_with = "serialize_span", deserialize_with = "deserialize_span")]
    pub span: Range<usize>,
}

/// One projected operation parameter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct IrisOperationParameter {
    /// Parameter author name.
    pub name: String,
    /// Canonical parameter type.
    pub type_expr: CanonicalType,
    /// Parameter byte span.
    #[serde(serialize_with = "serialize_span", deserialize_with = "deserialize_span")]
    pub span: Range<usize>,
}

/// Exact source text + byte span retained by VOS syntax projection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct IrisSyntaxSlice {
    /// Exact source text.
    pub text: String,
    /// Byte span within the source.
    #[serde(serialize_with = "serialize_span", deserialize_with = "deserialize_span")]
    pub span: Range<usize>,
}

// --- span wire ---------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SpanWire {
    start: usize,
    end: usize,
}

fn serialize_span<S: serde::Serializer>(span: &Range<usize>, serializer: S) -> Result<S::Ok, S::Error> {
    (span.start..span.end).serialize(serializer)
}

fn deserialize_span<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Range<usize>, D::Error> {
    let span = SpanWire::deserialize(deserializer)?;
    if span.start > span.end {
        return Err(serde::de::Error::custom("span start is after span end"));
    }
    Ok((span.start..span.end).into())
}

// --- validation --------------------------------------------------------------

/// VOS operation projection stage / version strings mirrored here so that
/// Iris's validator pins the exact values it accepts.
const ACCEPTED_STAGE: &str = "operation-syntax-projection";
const ACCEPTED_FORMAT_VERSION: &str = "vos-operation-contract-v0";
const ACCEPTED_LANGUAGE_VERSION: &str = "vos-language-v0";
const ACCEPTED_CANONICALIZATION_VERSION: &str = "vos-canonical-v0";

impl IrisOperationArtifact {
    /// Deserialize a V0 operation projection artifact from its JSON string
    /// representation and run Iris's compatibility validations.
    pub fn from_json(input: &str) -> Result<Self, OperationArtifactError> {
        let artifact: Self = serde_json::from_str(input).map_err(|e| OperationArtifactError::Decode(e.to_string()))?;
        artifact.validate()?;
        Ok(artifact)
    }

    /// Checks that the artifact's envelope / structural invariants match the
    /// version Iris knows how to consume.
    fn validate(&self) -> Result<(), OperationArtifactError> {
        let env = &self.envelope;
        if env.stage != ACCEPTED_STAGE
            || env.contract_format_version != ACCEPTED_FORMAT_VERSION
            || env.language_version != ACCEPTED_LANGUAGE_VERSION
            || env.canonicalization_version != ACCEPTED_CANONICALIZATION_VERSION
        {
            return Err(OperationArtifactError::Validate {
                code: "OPR011".into(),
                message: "unsupported operation projection version or stage".into(),
            });
        }
        if env.schema_fingerprint.is_some() || env.source_units.len() != 1 || env.source_units[0].source_unit_id != 0 {
            return Err(OperationArtifactError::Validate {
                code: "OPR012".into(),
                message: "invalid operation projection source unit or fingerprint state".into(),
            });
        }
        // Spot-check every operation has a non-empty name.
        for op in &self.operations {
            if op.name.is_empty() {
                return Err(OperationArtifactError::Validate { code: "OPR013".into(), message: "operation name cannot be empty".into() });
            }
        }
        Ok(())
    }

    /// Returns the schema fingerprint carried by the artifact envelope, if
    /// any is present. V0 leaves it `None` so Iris is allowed to fill it
    /// later by combining with the ResolvedContract fingerprint.
    pub fn schema_fingerprint(&self) -> Option<&str> {
        self.envelope.schema_fingerprint.as_deref()
    }

    /// Total number of operations in this artifact.
    pub fn len(&self) -> usize {
        self.operations.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_OP_ARTIFACT: &str = r#"{
        "envelope": {
            "stage": "operation-syntax-projection",
            "contractFormatVersion": "vos-operation-contract-v0",
            "languageVersion": "vos-language-v0",
            "canonicalizationVersion": "vos-canonical-v0",
            "schemaFingerprint": null,
            "sourceUnits": [
                { "sourceUnitId": 0, "source": "query get_user(id: uuid) -> User?" }
            ]
        },
        "operations": [
            {
                "kind": "query",
                "name": "get_user",
                "identityKey": "query:get_user(id:uuid)->User?",
                "operationId": "op_0123456789abcdef",
                "parameters": [
                    { "name": "id", "typeExpr": { "named": ["uuid"] }, "span": { "start": 15, "end": 23 } }
                ],
                "returnType": { "optional": { "named": ["User"] } },
                "signature": { "text": "get_user(id: uuid) -> User?", "span": { "start": 6, "end": 32 } },
                "body": null,
                "span": { "start": 0, "end": 50 }
            }
        ]
    }"#;

    #[test]
    fn from_json_parses_minimal_artifact() {
        let artifact = IrisOperationArtifact::from_json(MINIMAL_OP_ARTIFACT).unwrap();
        assert_eq!(artifact.len(), 1);
        let op = &artifact.operations[0];
        assert_eq!(op.name, "get_user");
        assert_eq!(op.kind, OperationKind::Query);
        assert_eq!(op.operation_id, "op_0123456789abcdef");
        assert_eq!(op.parameters.len(), 1);
        assert_eq!(op.parameters[0].name, "id");
    }

    #[test]
    fn from_json_rejects_wrong_stage() {
        let bad = r#"{
            "envelope": {
                "stage": "wrong-stage",
                "contractFormatVersion": "vos-operation-contract-v0",
                "languageVersion": "vos-language-v0",
                "canonicalizationVersion": "vos-canonical-v0",
                "schemaFingerprint": null,
                "sourceUnits": [
                    { "sourceUnitId": 0, "source": "" }
                ]
            },
            "operations": []
        }"#;
        let err = IrisOperationArtifact::from_json(bad).unwrap_err();
        assert!(matches!(err, OperationArtifactError::Validate { code, .. } if code == "OPR011"));
    }

    #[test]
    fn from_json_rejects_unknown_fields() {
        let bad = r#"{
            "envelope": {
                "stage": "operation-syntax-projection",
                "contractFormatVersion": "vos-operation-contract-v0",
                "languageVersion": "vos-language-v0",
                "canonicalizationVersion": "vos-canonical-v0",
                "schemaFingerprint": null,
                "sourceUnits": [
                    { "sourceUnitId": 0, "source": "" }
                ]
            },
            "operations": [],
            "totallyUnexpectedField": true
        }"#;
        assert!(IrisOperationArtifact::from_json(bad).is_err());
    }
}
