//! Local filesystem Search/Vector projection store (Phase 10-F).
//!
//! Rebuild fills a **new generation** while the active alias keeps serving.
//! Alias switch is atomic (rewrite `alias.von`). Never clear-and-refill the
//! live generation in place.

use std::fs;
use std::path::{Path, PathBuf};

use iris_ir::{ProjectionCandidate, ProjectionDocument, ProjectionGeneration};
use serde::{Deserialize, Serialize};

use crate::topology::ProjectionPolicy;

/// Format marker for alias documents.
pub const PROJECTION_ALIAS_FORMAT: &str = "iris.projection_alias";
/// Format marker for generation meta.
pub const PROJECTION_GENERATION_FORMAT: &str = "iris.projection_generation";

/// Generation lifecycle during rebuild.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationState {
    /// Being filled; not readable via alias.
    Building,
    /// Validated and eligible for alias switch.
    Ready,
    /// Active read target.
    Active,
    /// Failed rebuild; eligible for GC.
    Failed,
    /// Superseded after alias switch.
    Retired,
}

/// Errors from the local projection store.
#[derive(Debug)]
pub enum ProjectionStoreError {
    /// I/O failure.
    Io(std::io::Error),
    /// Policy / precondition.
    Policy(String),
    /// Missing generation / document.
    NotFound(String),
    /// VON / serialization.
    Codec(String),
}

impl std::fmt::Display for ProjectionStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "projection store io: {e}"),
            Self::Policy(s) | Self::NotFound(s) | Self::Codec(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for ProjectionStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ProjectionStoreError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Result alias.
pub type ProjectionStoreResult<T> = Result<T, ProjectionStoreError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AliasWire {
    format: String,
    version: i64,
    component: String,
    active_generation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GenerationMetaWire {
    format: String,
    version: i64,
    component: String,
    generation: String,
    state: GenerationState,
    schema_fingerprint: String,
    doc_count: u64,
    created_unix_ms: u64,
    updated_unix_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
}

/// Handle for an in-progress rebuild (building generation).
#[derive(Debug, Clone)]
pub struct RebuildHandle {
    /// Topology component id.
    pub component: String,
    /// New generation id.
    pub generation: ProjectionGeneration,
    /// Expected schema fingerprint for documents.
    pub schema_fingerprint: String,
}

/// Rebuild validation report (pre-activate).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebuildValidation {
    /// Generation validated.
    pub generation: String,
    /// Document count.
    pub doc_count: u64,
    /// Whether activate is allowed.
    pub ok: bool,
    /// Notes (no secrets).
    pub notes: Vec<String>,
}

/// Operator-facing rebuild status row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionRebuildStatus {
    /// Component id.
    pub component: String,
    /// Active generation, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_generation: Option<String>,
    /// Building generation, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub building_generation: Option<String>,
    /// Known generations.
    pub generations: Vec<String>,
    /// Notes.
    #[serde(default)]
    pub notes: Vec<String>,
}

/// Local projection store root (one root may host many components).
#[derive(Debug, Clone)]
pub struct LocalProjectionStore {
    root: PathBuf,
    policy: ProjectionPolicy,
}

impl LocalProjectionStore {
    /// Open or create a store under `root`.
    pub fn open(root: impl Into<PathBuf>, policy: ProjectionPolicy) -> ProjectionStoreResult<Self> {
        let root = root.into();
        fs::create_dir_all(root.join("components")).map_err(ProjectionStoreError::Io)?;
        Ok(Self { root, policy })
    }

    /// Store root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Begin a generation-isolated rebuild (does not touch the active alias).
    pub fn begin_rebuild(
        &self,
        component: &str,
        schema_fingerprint: &str,
        now_unix_ms: u64,
    ) -> ProjectionStoreResult<RebuildHandle> {
        self.ensure_component(component)?;
        let gen_id = format!("g{}", now_unix_ms);
        let generation = ProjectionGeneration(gen_id.clone());
        let dir = self.gen_dir(component, &gen_id);
        fs::create_dir_all(dir.join("docs")).map_err(ProjectionStoreError::Io)?;
        let meta = GenerationMetaWire {
            format: PROJECTION_GENERATION_FORMAT.into(),
            version: 1,
            component: component.into(),
            generation: gen_id.clone(),
            state: GenerationState::Building,
            schema_fingerprint: schema_fingerprint.into(),
            doc_count: 0,
            created_unix_ms: now_unix_ms,
            updated_unix_ms: now_unix_ms,
            notes: Some("building; not readable via alias".into()),
        };
        self.write_gen_meta(component, &gen_id, &meta)?;
        Ok(RebuildHandle {
            component: component.into(),
            generation,
            schema_fingerprint: schema_fingerprint.into(),
        })
    }

    /// Upsert a document into the building generation.
    pub fn upsert_building(
        &self,
        handle: &RebuildHandle,
        mut doc: ProjectionDocument,
        now_unix_ms: u64,
    ) -> ProjectionStoreResult<()> {
        let mut meta = self.load_gen_meta(&handle.component, handle.generation.as_str())?;
        if meta.state != GenerationState::Building {
            return Err(ProjectionStoreError::Policy(format!(
                "generation `{}` is {:?}, expected building",
                handle.generation, meta.state
            )));
        }
        if doc.schema_fingerprint != handle.schema_fingerprint {
            return Err(ProjectionStoreError::Policy(
                "document schema_fingerprint does not match rebuild handle".into(),
            ));
        }
        doc.generation = handle.generation.clone();
        let path = self.doc_path(
            &handle.component,
            handle.generation.as_str(),
            &doc.entity_id,
        );
        let existed = path.exists();
        let text = von::to_string_indented(&doc)
            .map_err(|e| ProjectionStoreError::Codec(format!("serialize projection doc: {e}")))?;
        fs::write(path, text).map_err(ProjectionStoreError::Io)?;
        if !existed {
            meta.doc_count = meta.doc_count.saturating_add(1);
        }
        meta.updated_unix_ms = now_unix_ms;
        self.write_gen_meta(&handle.component, handle.generation.as_str(), &meta)?;
        Ok(())
    }

    /// Validate a building generation before activate.
    pub fn validate_building(
        &self,
        handle: &RebuildHandle,
    ) -> ProjectionStoreResult<RebuildValidation> {
        let meta = self.load_gen_meta(&handle.component, handle.generation.as_str())?;
        let mut notes = Vec::new();
        let mut ok = meta.state == GenerationState::Building;
        if !ok {
            notes.push(format!("state is {:?}, expected building", meta.state));
        }
        if self.policy.require_nonempty_rebuild && meta.doc_count == 0 {
            ok = false;
            notes.push("require_nonempty_rebuild: doc_count is 0".into());
        }
        if meta.schema_fingerprint != handle.schema_fingerprint {
            ok = false;
            notes.push("schema_fingerprint mismatch vs handle".into());
        }
        if ok {
            notes.push("validation ok; safe to activate (alias switch)".into());
        }
        Ok(RebuildValidation {
            generation: handle.generation.as_str().into(),
            doc_count: meta.doc_count,
            ok,
            notes,
        })
    }

    /// Mark building 鈫?ready then atomically switch the read alias.
    ///
    /// Previous active generation becomes `retired`. Never mutates docs under
    /// the previous active generation during switch.
    pub fn activate(&self, handle: &RebuildHandle, now_unix_ms: u64) -> ProjectionStoreResult<()> {
        let validation = self.validate_building(handle)?;
        if !validation.ok {
            return Err(ProjectionStoreError::Policy(format!(
                "rebuild validation failed: {}",
                validation.notes.join("; ")
            )));
        }
        let mut meta = self.load_gen_meta(&handle.component, handle.generation.as_str())?;
        meta.state = GenerationState::Ready;
        meta.updated_unix_ms = now_unix_ms;
        self.write_gen_meta(&handle.component, handle.generation.as_str(), &meta)?;

        let prev = self.active_generation(&handle.component)?;
        // Atomic-ish alias rewrite (single file replace).
        let alias = AliasWire {
            format: PROJECTION_ALIAS_FORMAT.into(),
            version: 1,
            component: handle.component.clone(),
            active_generation: Some(handle.generation.as_str().into()),
        };
        self.write_alias(&handle.component, &alias)?;

        meta.state = GenerationState::Active;
        meta.updated_unix_ms = now_unix_ms;
        meta.notes = Some("active via alias switch".into());
        self.write_gen_meta(&handle.component, handle.generation.as_str(), &meta)?;

        if let Some(prev_gen) = prev {
            if prev_gen != handle.generation.as_str() {
                if let Ok(mut old) = self.load_gen_meta(&handle.component, &prev_gen) {
                    old.state = GenerationState::Retired;
                    old.updated_unix_ms = now_unix_ms;
                    old.notes = Some("retired after alias switch".into());
                    let _ = self.write_gen_meta(&handle.component, &prev_gen, &old);
                }
            }
        }
        self.gc_retired(&handle.component)?;
        Ok(())
    }

    /// Abort a building generation (mark failed).
    pub fn abort_rebuild(
        &self,
        handle: &RebuildHandle,
        now_unix_ms: u64,
    ) -> ProjectionStoreResult<()> {
        let mut meta = self.load_gen_meta(&handle.component, handle.generation.as_str())?;
        if meta.state != GenerationState::Building {
            return Err(ProjectionStoreError::Policy(
                "abort_rebuild only applies to building generations".into(),
            ));
        }
        meta.state = GenerationState::Failed;
        meta.updated_unix_ms = now_unix_ms;
        meta.notes = Some("aborted rebuild".into());
        self.write_gen_meta(&handle.component, handle.generation.as_str(), &meta)?;
        Ok(())
    }

    /// Active generation id for a component.
    pub fn active_generation(&self, component: &str) -> ProjectionStoreResult<Option<String>> {
        let path = self.alias_path(component);
        if !path.exists() {
            return Ok(None);
        }
        let text = fs::read_to_string(&path)?;
        let alias: AliasWire = von::from_str(&text)
            .map_err(|e| ProjectionStoreError::Codec(format!("corrupt alias: {e}")))?;
        Ok(alias.active_generation)
    }

    /// Full-text-ish candidate search over the **active** generation.
    ///
    /// Returns candidates only 鈥?callers must hydrate through Authority.
    pub fn search(
        &self,
        component: &str,
        query: &str,
        limit: usize,
    ) -> ProjectionStoreResult<Vec<ProjectionCandidate>> {
        let Some(gen_id) = self.active_generation(component)? else {
            return Err(ProjectionStoreError::NotFound(format!(
                "no active generation for `{component}`"
            )));
        };
        let q = query.to_ascii_lowercase();
        let tokens: Vec<_> = q
            .split(|c: char| !c.is_alphanumeric())
            .filter(|t| !t.is_empty())
            .collect();
        let mut hits = Vec::new();
        for doc in self.load_docs(component, &gen_id)? {
            let hay = doc.text.clone().unwrap_or_default().to_ascii_lowercase();
            let field_blob = doc
                .fields
                .values()
                .cloned()
                .collect::<Vec<_>>()
                .join(" ")
                .to_ascii_lowercase();
            let blob = format!("{hay} {field_blob}");
            let score = if tokens.is_empty() {
                0.0
            } else {
                tokens.iter().filter(|t| blob.contains(*t)).count() as f64 / tokens.len() as f64
            };
            if score > 0.0 {
                hits.push(ProjectionCandidate {
                    entity_id: doc.entity_id,
                    score,
                    entity_version: doc.entity_version,
                    generation: ProjectionGeneration(gen_id.clone()),
                    schema_fingerprint: doc.schema_fingerprint,
                });
            }
        }
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.entity_id.cmp(&b.entity_id))
        });
        hits.truncate(limit);
        Ok(hits)
    }

    /// Brute-force nearest over active generation vectors (L2 鈫?score = 1/(1+d)).
    pub fn nearest(
        &self,
        component: &str,
        query: &[f32],
        k: usize,
    ) -> ProjectionStoreResult<Vec<ProjectionCandidate>> {
        let Some(gen_id) = self.active_generation(component)? else {
            return Err(ProjectionStoreError::NotFound(format!(
                "no active generation for `{component}`"
            )));
        };
        let mut hits = Vec::new();
        for doc in self.load_docs(component, &gen_id)? {
            let Some(ref vec_s) = doc.vector else {
                continue;
            };
            let Ok(vec) = parse_vector(vec_s) else {
                continue;
            };
            if vec.len() != query.len() {
                continue;
            }
            let dist = l2(query, &vec);
            let score = 1.0 / (1.0 + dist);
            hits.push(ProjectionCandidate {
                entity_id: doc.entity_id,
                score,
                entity_version: doc.entity_version,
                generation: ProjectionGeneration(gen_id.clone()),
                schema_fingerprint: doc.schema_fingerprint,
            });
        }
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.entity_id.cmp(&b.entity_id))
        });
        hits.truncate(k);
        Ok(hits)
    }

    /// Rebuild/status snapshot for a component.
    pub fn rebuild_status(
        &self,
        component: &str,
    ) -> ProjectionStoreResult<ProjectionRebuildStatus> {
        self.ensure_component(component)?;
        let active = self.active_generation(component)?;
        let mut generations = Vec::new();
        let mut building = None;
        let gens_root = self.component_dir(component).join("generations");
        if gens_root.exists() {
            for entry in fs::read_dir(&gens_root)? {
                let entry = entry?;
                let name = entry.file_name();
                let Some(gen_id) = name.to_str() else {
                    continue;
                };
                generations.push(gen_id.to_string());
                if let Ok(meta) = self.load_gen_meta(component, gen_id) {
                    if meta.state == GenerationState::Building {
                        building = Some(gen_id.to_string());
                    }
                }
            }
        }
        generations.sort();
        Ok(ProjectionRebuildStatus {
            component: component.into(),
            active_generation: active,
            building_generation: building,
            generations,
            notes: vec![
                "rebuild fills a new generation; alias switch is atomic".into(),
                "search/vector hits are candidates; hydrate via authority".into(),
            ],
        })
    }

    fn gc_retired(&self, component: &str) -> ProjectionStoreResult<()> {
        let keep = self.policy.keep_generations.unwrap_or(2).max(1) as usize;
        let mut retired = Vec::new();
        let gens_root = self.component_dir(component).join("generations");
        if !gens_root.exists() {
            return Ok(());
        }
        for entry in fs::read_dir(&gens_root)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(gen_id) = name.to_str() else {
                continue;
            };
            if let Ok(meta) = self.load_gen_meta(component, gen_id) {
                if matches!(
                    meta.state,
                    GenerationState::Retired | GenerationState::Failed
                ) {
                    retired.push((gen_id.to_string(), meta.updated_unix_ms));
                }
            }
        }
        retired.sort_by(|a, b| b.1.cmp(&a.1));
        for (gen_id, _) in retired.into_iter().skip(keep) {
            let _ = fs::remove_dir_all(self.gen_dir(component, &gen_id));
        }
        Ok(())
    }

    fn ensure_component(&self, component: &str) -> ProjectionStoreResult<()> {
        fs::create_dir_all(self.component_dir(component).join("generations"))?;
        let alias = self.alias_path(component);
        if !alias.exists() {
            let wire = AliasWire {
                format: PROJECTION_ALIAS_FORMAT.into(),
                version: 1,
                component: component.into(),
                active_generation: None,
            };
            self.write_alias(component, &wire)?;
        }
        Ok(())
    }

    fn component_dir(&self, component: &str) -> PathBuf {
        self.root.join("components").join(component)
    }

    fn alias_path(&self, component: &str) -> PathBuf {
        self.component_dir(component).join("alias.von")
    }

    fn gen_dir(&self, component: &str, gen_id: &str) -> PathBuf {
        self.component_dir(component)
            .join("generations")
            .join(gen_id)
    }

    fn doc_path(&self, component: &str, gen_id: &str, entity_id: &str) -> PathBuf {
        self.gen_dir(component, gen_id)
            .join("docs")
            .join(format!("{entity_id}.von"))
    }

    fn write_alias(&self, component: &str, alias: &AliasWire) -> ProjectionStoreResult<()> {
        let text = von::to_string_indented(alias)
            .map_err(|e| ProjectionStoreError::Codec(format!("serialize alias: {e}")))?;
        let path = self.alias_path(component);
        let tmp = path.with_extension("von.tmp");
        fs::write(&tmp, &text)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }

    fn write_gen_meta(
        &self,
        component: &str,
        gen_id: &str,
        meta: &GenerationMetaWire,
    ) -> ProjectionStoreResult<()> {
        let text = von::to_string_indented(meta)
            .map_err(|e| ProjectionStoreError::Codec(format!("serialize generation meta: {e}")))?;
        fs::write(self.gen_dir(component, gen_id).join("meta.von"), text)?;
        Ok(())
    }

    fn load_gen_meta(
        &self,
        component: &str,
        gen_id: &str,
    ) -> ProjectionStoreResult<GenerationMetaWire> {
        let path = self.gen_dir(component, gen_id).join("meta.von");
        if !path.exists() {
            return Err(ProjectionStoreError::NotFound(format!(
                "generation `{gen_id}` not found for `{component}`"
            )));
        }
        let text = fs::read_to_string(path)?;
        von::from_str(&text)
            .map_err(|e| ProjectionStoreError::Codec(format!("corrupt generation meta: {e}")))
    }

    fn load_docs(
        &self,
        component: &str,
        gen_id: &str,
    ) -> ProjectionStoreResult<Vec<ProjectionDocument>> {
        let dir = self.gen_dir(component, gen_id).join("docs");
        let mut out = Vec::new();
        if !dir.exists() {
            return Ok(out);
        }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let text = fs::read_to_string(entry.path())?;
            let doc: ProjectionDocument = von::from_str(&text)
                .map_err(|e| ProjectionStoreError::Codec(format!("corrupt projection doc: {e}")))?;
            out.push(doc);
        }
        Ok(out)
    }
}

fn parse_vector(parts: &[String]) -> Result<Vec<f32>, ()> {
    parts
        .iter()
        .map(|s| s.parse::<f32>().map_err(|_| ()))
        .collect()
}

fn l2(a: &[f32], b: &[f32]) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| {
            let d = f64::from(*x) - f64::from(*y);
            d * d
        })
        .sum::<f64>()
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hydrate::{AuthorityEntity, MapAuthorityLookup, hydrate_candidates};
    use iris_ir::ProjectionGeneration;

    fn tmp() -> (LocalProjectionStore, PathBuf) {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("iris-proj-{stamp}"));
        let store = LocalProjectionStore::open(
            &root,
            ProjectionPolicy {
                max_lag_secs: Some(60),
                keep_generations: Some(2),
                require_nonempty_rebuild: true,
                covered_fields: vec!["title".into()],
            },
        )
        .unwrap();
        (store, root)
    }

    #[test]
    fn rebuild_isolates_generation_then_switches_alias() {
        let (store, root) = tmp();
        let h1 = store.begin_rebuild("search", "fp1", 1_000).unwrap();
        store
            .upsert_building(
                &h1,
                ProjectionDocument {
                    entity_id: "u1".into(),
                    entity_version: 1,
                    schema_fingerprint: "fp1".into(),
                    generation: ProjectionGeneration("x".into()),
                    text: Some("hello world".into()),
                    vector: None,
                    fields: Default::default(),
                },
                1_001,
            )
            .unwrap();
        store.activate(&h1, 1_002).unwrap();
        assert_eq!(
            store.active_generation("search").unwrap().as_deref(),
            Some(h1.generation.as_str())
        );

        // Live search works on g1 while g2 builds.
        let hits = store.search("search", "hello", 10).unwrap();
        assert_eq!(hits.len(), 1);

        let h2 = store.begin_rebuild("search", "fp1", 2_000).unwrap();
        store
            .upsert_building(
                &h2,
                ProjectionDocument {
                    entity_id: "u1".into(),
                    entity_version: 2,
                    schema_fingerprint: "fp1".into(),
                    generation: ProjectionGeneration("x".into()),
                    text: Some("hello rebuilt".into()),
                    vector: None,
                    fields: Default::default(),
                },
                2_001,
            )
            .unwrap();
        // Still serving old generation until activate.
        let hits = store.search("search", "rebuilt", 10).unwrap();
        assert!(hits.is_empty());
        store.activate(&h2, 2_002).unwrap();
        let hits = store.search("search", "rebuilt", 10).unwrap();
        assert_eq!(hits[0].entity_version, 2);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn search_candidates_hydrate_through_authority() {
        let (store, root) = tmp();
        let h = store.begin_rebuild("search", "fp", 3_000).unwrap();
        store
            .upsert_building(
                &h,
                ProjectionDocument {
                    entity_id: "a".into(),
                    entity_version: 1,
                    schema_fingerprint: "fp".into(),
                    generation: ProjectionGeneration("x".into()),
                    text: Some("alpha beta".into()),
                    vector: None,
                    fields: Default::default(),
                },
                3_001,
            )
            .unwrap();
        store
            .upsert_building(
                &h,
                ProjectionDocument {
                    entity_id: "ghost".into(),
                    entity_version: 1,
                    schema_fingerprint: "fp".into(),
                    generation: ProjectionGeneration("x".into()),
                    text: Some("alpha ghost".into()),
                    vector: None,
                    fields: Default::default(),
                },
                3_002,
            )
            .unwrap();
        store.activate(&h, 3_003).unwrap();

        let cands = store.search("search", "alpha", 10).unwrap();
        assert_eq!(cands.len(), 2);

        let mut auth = MapAuthorityLookup::default();
        auth.rows.insert(
            "a".into(),
            AuthorityEntity {
                entity_id: "a".into(),
                entity_version: 1,
                deleted: false,
                payload: Some("row-a".into()),
            },
        );
        // ghost missing -> dropped
        let hydrated = hydrate_candidates(&cands, &auth).unwrap();
        assert_eq!(hydrated.entities.len(), 1);
        assert_eq!(hydrated.entities[0].entity_id, "a");
        assert_eq!(hydrated.completeness.dropped_missing, 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn vector_nearest_returns_ranked_candidates() {
        let (store, root) = tmp();
        let h = store.begin_rebuild("vecs", "fp", 4_000).unwrap();
        for (id, vec) in [
            ("near", vec!["1".into(), "0".into()]),
            ("far", vec!["0".into(), "1".into()]),
        ] {
            store
                .upsert_building(
                    &h,
                    ProjectionDocument {
                        entity_id: id.into(),
                        entity_version: 1,
                        schema_fingerprint: "fp".into(),
                        generation: ProjectionGeneration("x".into()),
                        text: None,
                        vector: Some(vec),
                        fields: Default::default(),
                    },
                    4_001,
                )
                .unwrap();
        }
        store.activate(&h, 4_002).unwrap();
        let hits = store.nearest("vecs", &[1.0, 0.0], 2).unwrap();
        assert_eq!(hits[0].entity_id, "near");
        assert!(hits[0].score >= hits[1].score);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cannot_activate_empty_when_required() {
        let (store, root) = tmp();
        let h = store.begin_rebuild("search", "fp", 5_000).unwrap();
        let err = store.activate(&h, 5_001).unwrap_err();
        assert!(err.to_string().contains("nonempty") || err.to_string().contains("validation"));
        let _ = fs::remove_dir_all(root);
    }
}
