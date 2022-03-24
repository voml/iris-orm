//! Versioned IR envelope carried to connectors/adapters.

use serde::{Deserialize, Serialize};

use crate::SemanticHash;
use crate::op::PhysicalOp;

/// Major.minor IR envelope version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IrVersion {
    /// Incompatible envelope changes.
    pub major: u16,
    /// Compatible additive changes.
    pub minor: u16,
}

impl IrVersion {
    /// Provisional Phase 1 envelope (not a frozen public major yet).
    pub const PHASE1: Self = Self { major: 0, minor: 1 };

    /// Accept when majors match and `other.minor <= self.minor` for readers
    /// that declare this version as maximum supported.
    pub fn accepts(self, other: Self) -> bool {
        self.major == other.major && other.minor <= self.minor
    }
}

/// Logical effect class for an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EffectKind {
    /// Pure / read path.
    Read,
    /// Row mutations.
    Write,
    /// Schema / catalog mutations.
    Schema,
}

/// Schema contract fingerprint (hex or opaque token).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SchemaFingerprint(pub String);

impl SchemaFingerprint {
    /// Empty / unbound fingerprint for Phase 1 reference stores.
    pub fn unbound() -> Self {
        Self("unbound".into())
    }
}

/// Envelope required on every connector/adapter input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IrEnvelope {
    /// VOS contract version string (provisional until VOS freezes IR).
    pub vos_contract_version: String,
    /// Iris physical IR version.
    pub ir_version: IrVersion,
    /// Schema fingerprint.
    pub schema_fingerprint: SchemaFingerprint,
    /// Stable operation id (caller or derived).
    pub operation_id: String,
    /// Effect class.
    pub effect: EffectKind,
    /// Capability ids required to execute natively.
    pub required_capabilities: Vec<String>,
    /// Source byte span of the root operation.
    pub span_start: usize,
    /// Source byte span end (exclusive).
    pub span_end: usize,
    /// Deterministic semantic hash of the logical op tree.
    pub semantic_hash: SemanticHash,
}

impl IrEnvelope {
    /// Validate this envelope against a supported IR version.
    pub fn check_version(&self, supported: IrVersion) -> crate::Result<()> {
        if supported.major != self.ir_version.major {
            return Err(crate::Error::UnsupportedVersion(
                self.ir_version.major,
                self.ir_version.minor,
            ));
        }
        if self.ir_version.minor > supported.minor {
            return Err(crate::Error::UnsupportedVersion(
                self.ir_version.major,
                self.ir_version.minor,
            ));
        }
        Ok(())
    }
}

/// Hash a physical op list deterministically for envelope stamping.
pub fn hash_ops(ops: &[PhysicalOp]) -> SemanticHash {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    // Debug is stable enough for Phase 1 local conformance (not a crypto seal).
    format!("{ops:?}").hash(&mut h);
    SemanticHash(h.finish())
}
