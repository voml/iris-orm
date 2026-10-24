//! Strict consumer for the `vos/iris.contract/v0` artifact envelope.
//!
//! Iris never parses a VOS body and never derives a second identity. It reads
//! the versioned artifact produced by `vos-language`, rejects anything that is
//! not an Iris `/v0` envelope, and exposes the validated contracts.

use vos::types::{
    ArtifactError, ContractArtifact, ContractFormat, ContractPayload, ContractProducer, OperationContract, OperationId, ServiceContract,
    ServiceId,
};

/// Contract format accepted by this consumer.
pub const ACCEPTED_CONTRACT_FORMAT: &str = "vos/iris.contract/v0";

/// Diagnostic code for an unreadable, unknown-version or foreign-format envelope.
pub const CODE_ENVELOPE: &str = "ART010";
/// Diagnostic code for a fingerprint that does not match the payload.
pub const CODE_FINGERPRINT: &str = "ART020";
/// Diagnostic code for a missing producer name or version.
pub const CODE_PRODUCER: &str = "ART021";

/// Structured contract consumption failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractError {
    /// Stable diagnostic code (`ART010`, `ART020`, `ART021`, or an upstream
    /// operation/service code kept verbatim).
    pub code: String,
    /// Human-readable reason.
    pub message: String,
}

impl ContractError {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self { code: code.to_owned(), message: message.into() }
    }

    fn from_artifact(error: ArtifactError) -> Self {
        Self { code: error.code, message: error.message }
    }
}

impl core::fmt::Display for ContractError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ContractError {}

/// A validated Iris contract artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrisContract {
    artifact: ContractArtifact,
}

impl IrisContract {
    /// Reads a strict `vos/iris.contract/v0` envelope.
    ///
    /// Unknown versions, foreign product formats, unknown fields and missing
    /// producer provenance are rejected before any payload is exposed.
    pub fn from_json(input: &str) -> Result<Self, ContractError> {
        let probe: serde_json::Value = serde_json::from_str(input)
            .map_err(|error| ContractError::new(CODE_ENVELOPE, format!("contract envelope is not valid JSON: {error}")))?;
        let version = probe
            .get("version")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| ContractError::new(CODE_ENVELOPE, "contract envelope is missing a string `version` field"))?;
        if version != ACCEPTED_CONTRACT_FORMAT {
            return Err(ContractError::new(
                CODE_ENVELOPE,
                format!("contract envelope version `{version}` is not accepted, expected `{ACCEPTED_CONTRACT_FORMAT}`"),
            ));
        }
        let producer = probe
            .get("producer")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| ContractError::new(CODE_PRODUCER, "contract envelope producer must carry a name and a version"))?;
        for field in ["name", "version"] {
            let present = producer.get(field).and_then(serde_json::Value::as_str).is_some_and(|value| !value.trim().is_empty());
            if !present {
                return Err(ContractError::new(CODE_PRODUCER, format!("contract envelope producer must carry a non-empty `{field}`")));
            }
        }
        let artifact = ContractArtifact::from_json(input).map_err(ContractError::from_artifact)?;
        Self::from_artifact(artifact)
    }

    /// Validates an already-deserialized artifact for Iris consumption.
    pub fn from_artifact(artifact: ContractArtifact) -> Result<Self, ContractError> {
        if artifact.version != ContractFormat::Iris {
            return Err(ContractError::new(
                CODE_ENVELOPE,
                format!("artifact version `{}` is not accepted, expected `{ACCEPTED_CONTRACT_FORMAT}`", artifact.version),
            ));
        }
        artifact.validate().map_err(ContractError::from_artifact)?;
        Ok(Self { artifact })
    }

    /// Envelope format of this artifact.
    pub fn format(&self) -> ContractFormat {
        self.artifact.version
    }

    /// Canonical payload fingerprint.
    pub fn fingerprint(&self) -> &str {
        &self.artifact.fingerprint
    }

    /// Producer provenance.
    pub fn producer(&self) -> &ContractProducer {
        &self.artifact.producer
    }

    /// Validated payload.
    pub fn payload(&self) -> &ContractPayload {
        &self.artifact.payload
    }

    /// Validated operation contracts.
    pub fn operations(&self) -> &[OperationContract] {
        &self.artifact.payload.operations
    }

    /// Validated service contracts.
    pub fn services(&self) -> &[ServiceContract] {
        &self.artifact.payload.services
    }

    /// Looks up an operation by durable identity.
    pub fn operation(&self, id: &OperationId) -> Option<&OperationContract> {
        self.operations().iter().find(|operation| &operation.operation_id == id)
    }

    /// Looks up a service by durable identity.
    pub fn service(&self, id: &ServiceId) -> Option<&ServiceContract> {
        self.services().iter().find(|service| &service.service_id == id)
    }
}
