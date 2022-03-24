//! Authority hydrate for search/vector candidates (Phase 10-F).
//!
//! Candidates are never treated as authoritative rows. Deleted / version-mismatched
//! hits are dropped with completeness metadata -- no ghost records.

use iris_ir::{
    HydrateCompleteness, HydrateDropReason, HydrateResult, HydratedEntity, ProjectionCandidate,
};

/// Authority-side entity snapshot used during hydrate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityEntity {
    /// Primary identity.
    pub entity_id: String,
    /// Monotonic entity version.
    pub entity_version: u64,
    /// Soft-deleted on authority.
    pub deleted: bool,
    /// Optional payload (coordinator / tests).
    pub payload: Option<String>,
}

/// Lookup identities on the Authority (implemented by adapters / test doubles).
pub trait AuthorityEntityLookup {
    /// Fetch one entity by primary id.
    fn lookup(&self, entity_id: &str) -> Result<Option<AuthorityEntity>, String>;
}

/// In-memory authority map for tests and offline rebuild seeds.
#[derive(Debug, Default, Clone)]
pub struct MapAuthorityLookup {
    /// entity_id -> row.
    pub rows: std::collections::BTreeMap<String, AuthorityEntity>,
}

impl AuthorityEntityLookup for MapAuthorityLookup {
    fn lookup(&self, entity_id: &str) -> Result<Option<AuthorityEntity>, String> {
        Ok(self.rows.get(entity_id).cloned())
    }
}

/// Hydrate search/vector candidates through Authority.
///
/// Policy: drop missing/deleted/version-mismatched candidates; do not invent
/// rows; mark `complete` when every candidate was considered (no truncation).
pub fn hydrate_candidates(
    candidates: &[ProjectionCandidate],
    authority: &dyn AuthorityEntityLookup,
) -> Result<HydrateResult, String> {
    let mut entities = Vec::new();
    let mut drops = Vec::new();
    let mut completeness = HydrateCompleteness {
        candidates: candidates.len() as u64,
        complete: true,
        ..HydrateCompleteness::default()
    };

    for c in candidates {
        match authority.lookup(&c.entity_id)? {
            None => {
                completeness.dropped_missing += 1;
                drops.push((c.entity_id.clone(), HydrateDropReason::DeletedOrMissing));
            }
            Some(row) if row.deleted => {
                completeness.dropped_deleted += 1;
                drops.push((c.entity_id.clone(), HydrateDropReason::AuthorityDeleted));
            }
            Some(row) if row.entity_version != c.entity_version => {
                completeness.dropped_version += 1;
                drops.push((c.entity_id.clone(), HydrateDropReason::VersionMismatch));
            }
            Some(row) => {
                completeness.hydrated += 1;
                entities.push(HydratedEntity {
                    entity_id: row.entity_id,
                    entity_version: row.entity_version,
                    payload: row.payload,
                });
            }
        }
    }

    Ok(HydrateResult {
        entities,
        completeness,
        drops,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use iris_ir::ProjectionGeneration;

    fn cand(id: &str, version: u64) -> ProjectionCandidate {
        ProjectionCandidate {
            entity_id: id.into(),
            score: 1.0,
            entity_version: version,
            generation: ProjectionGeneration("g1".into()),
            schema_fingerprint: "fp".into(),
        }
    }

    #[test]
    fn drops_missing_deleted_and_stale() {
        let mut auth = MapAuthorityLookup::default();
        auth.rows.insert(
            "a".into(),
            AuthorityEntity {
                entity_id: "a".into(),
                entity_version: 2,
                deleted: false,
                payload: Some("ok".into()),
            },
        );
        auth.rows.insert(
            "b".into(),
            AuthorityEntity {
                entity_id: "b".into(),
                entity_version: 1,
                deleted: true,
                payload: None,
            },
        );
        auth.rows.insert(
            "c".into(),
            AuthorityEntity {
                entity_id: "c".into(),
                entity_version: 9,
                deleted: false,
                payload: None,
            },
        );

        let result = hydrate_candidates(
            &[cand("a", 2), cand("b", 1), cand("c", 1), cand("d", 1)],
            &auth,
        )
        .unwrap();
        assert_eq!(result.entities.len(), 1);
        assert_eq!(result.entities[0].entity_id, "a");
        assert_eq!(result.completeness.hydrated, 1);
        assert_eq!(result.completeness.dropped_deleted, 1);
        assert_eq!(result.completeness.dropped_version, 1);
        assert_eq!(result.completeness.dropped_missing, 1);
        assert!(result.completeness.complete);
    }
}
