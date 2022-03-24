//! Search / vector projection candidates and hydrate shapes (Phase 10-F).
//!
//! Projections return **candidates** (identity + score + versions), never
//! authoritative entity bodies. Callers must hydrate through Authority.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Projection generation id (isolated rebuild target).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProjectionGeneration(pub String);

impl ProjectionGeneration {
    /// Borrow raw id.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ProjectionGeneration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Why a search/vector candidate was dropped during hydrate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HydrateDropReason {
    /// Entity missing on authority (deleted or never existed).
    DeletedOrMissing,
    /// Candidate entity_version older/newer than authority truth.
    VersionMismatch,
    /// Authority marked the row deleted.
    AuthorityDeleted,
}

/// Candidate hit from a search or vector projection (not an authoritative row).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectionCandidate {
    /// VOS primary identity.
    pub entity_id: String,
    /// Ranking score (higher is better for search; distance inverted for vector).
    pub score: f64,
    /// Entity version observed in the projection document.
    pub entity_version: u64,
    /// Projection generation that produced the candidate.
    pub generation: ProjectionGeneration,
    /// Schema fingerprint stamped on the projection doc.
    pub schema_fingerprint: String,
}

/// Document stored inside a projection generation (rebuild fill unit).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectionDocument {
    /// VOS primary identity.
    pub entity_id: String,
    /// Authority entity version at index time.
    pub entity_version: u64,
    /// Schema fingerprint.
    pub schema_fingerprint: String,
    /// Generation this document belongs to.
    pub generation: ProjectionGeneration,
    /// Optional searchable text blob.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Optional dense vector for nearest search (decimal strings -- VON has no floats).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vector: Option<Vec<String>>,
    /// Optional field map (covered fields for search).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, String>,
}

/// One hydrated authoritative entity after candidate validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HydratedEntity {
    /// VOS primary identity.
    pub entity_id: String,
    /// Authority entity version.
    pub entity_version: u64,
    /// Opaque authority payload (tests / coordinators; not SQL).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<String>,
}

/// Completeness metadata for a hydrate batch (no partial by default).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HydrateCompleteness {
    /// Candidates considered.
    pub candidates: u64,
    /// Entities returned after hydrate.
    pub hydrated: u64,
    /// Dropped as deleted/missing.
    pub dropped_missing: u64,
    /// Dropped for version mismatch.
    pub dropped_version: u64,
    /// Dropped because authority marked deleted.
    pub dropped_deleted: u64,
    /// True when results are complete per policy (no silent truncation).
    pub complete: bool,
}

/// Full hydrate outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HydrateResult {
    /// Authoritative entities in candidate order (minus drops).
    pub entities: Vec<HydratedEntity>,
    /// Completeness counters.
    pub completeness: HydrateCompleteness,
    /// Dropped candidate ids with reasons.
    pub drops: Vec<(String, HydrateDropReason)>,
}
