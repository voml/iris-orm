//! Composite Backend topology contract (Phase 10-A).
//!
//! Offline load / validate / plan only. No live multi-middleware execution.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use iris_ir::{
    AccessKind, COMPOSITE_PLAN_FORMAT, CompositePlan, CompositeStep, ConsistencyIntent, RouteProof,
};
use serde::{Deserialize, Serialize};

/// Filename convention directory for topology documents.
pub const TOPOLOGY_DIR: &str = "topologies";

/// Stable discriminator for topology documents.
pub const TOPOLOGY_FORMAT: &str = "iris.topology";

/// Role a middleware component may play in a composite datasource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentRole {
    /// Sole data truth for bound tables.
    Authority,
    /// Disposable identity/query cache.
    Cache,
    /// Full-text / search projection.
    SearchProjection,
    /// ANN / vector projection.
    VectorProjection,
    /// Bytes / file object store.
    ObjectStore,
    /// Durable outbox / event log.
    Outbox,
    /// Async work queue (not truth).
    Queue,
    /// Read-only authority replica.
    Replica,
    /// Lock / lease coordinator.
    Lock,
}

impl ComponentRole {
    /// True when this role may be declared as table authority.
    pub fn is_authority(self) -> bool {
        matches!(self, Self::Authority)
    }

    /// True when role is a derived reader.
    pub fn is_derived_reader(self) -> bool {
        matches!(
            self,
            Self::Cache | Self::SearchProjection | Self::VectorProjection | Self::Replica
        )
    }
}

/// One named middleware component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologyComponent {
    /// Role in the composite.
    pub role: ComponentRole,
    /// Adapter id (e.g. `postgres`, `redis`) ? never a secret.
    pub adapter: String,
    /// Optional adapter version label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter_version: Option<String>,
    /// Datasource name in `iris.von` this component binds to (optional in 10-A).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub datasource: Option<String>,
}

/// Cache policy (declared on topology; stampede enforced in coordinators via
/// [`crate::StampedeBudget`], Phase 10-C).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CachePolicy {
    /// TTL seconds (correctness still requires watermarks; TTL is a floor).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl_secs: Option<u64>,
    /// Negative-cache TTL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_ttl_secs: Option<u64>,
    /// Stampede / singleflight concurrency budget for authority fill-on-miss.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stampede_budget: Option<u32>,
}

/// Outbox policy (Phase 10-A: declared, not executed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct OutboxPolicy {
    /// Partition / ordering domain label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ordering_domain: Option<String>,
    /// Deduplication key strategy label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dedupe_key: Option<String>,
    /// Dead-letter policy label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dead_letter: Option<String>,
}

/// Object store lifecycle / GC policy (Phase 10-E).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ObjectPolicy {
    /// Content-hash algorithm label (`blake3` default in FsObjectStore).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash_alg: Option<String>,
    /// Orphan TTL (seconds) for pending/verified without finalize.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orphan_ttl_secs: Option<u64>,
    /// Pending-specific TTL override (falls back to orphan_ttl_secs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_ttl_secs: Option<u64>,
}

/// Search / vector projection rebuild + coverage policy (Phase 10-F).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProjectionPolicy {
    /// Maximum acceptable projection lag (seconds) for BoundedStale routes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_lag_secs: Option<u64>,
    /// How many retired/failed generations to retain after alias switch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_generations: Option<u32>,
    /// When true, rebuild activate refuses an empty building generation.
    #[serde(default)]
    pub require_nonempty_rebuild: bool,
    /// Declared covered fields for search pushdown (labels only in 10-F).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub covered_fields: Vec<String>,
}

/// Fallback when a component is unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FallbackPolicy {
    /// Fall back to authority (typical for cache).
    Authority,
    /// Fail closed.
    FailClosed,
}

/// Per-access-kind default route declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteRule {
    /// Default consistency when caller does not override.
    pub default_intent: ConsistencyIntent,
    /// Preferred derived component for this access (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_component: Option<String>,
    /// Fallback when preferred path cannot be proven.
    #[serde(default = "default_fallback")]
    pub fallback: FallbackPolicy,
}

fn default_fallback() -> FallbackPolicy {
    FallbackPolicy::Authority
}

/// Table -> authority binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableBinding {
    /// Authority component id (must have role=authority).
    pub authority: String,
}

/// Versioned topology contract for one logical composite datasource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologyContract {
    /// Discriminator (`iris.topology`).
    pub format: String,
    /// Document schema version.
    pub version: i64,
    /// Logical datasource / topology id.
    pub id: String,
    /// Topology revision (handshake / activate).
    pub topology_version: i64,
    /// Named components.
    pub components: BTreeMap<String, TopologyComponent>,
    /// Table bindings (must each point at exactly one authority component).
    #[serde(default)]
    pub tables: BTreeMap<String, TableBinding>,
    /// Access-kind routes.
    #[serde(default)]
    pub routes: BTreeMap<String, RouteRule>,
    /// Cache policy (applies to cache-role components).
    #[serde(default)]
    pub cache: CachePolicy,
    /// Outbox policy.
    #[serde(default)]
    pub outbox: OutboxPolicy,
    /// Object store lifecycle / GC policy.
    #[serde(default)]
    pub object: ObjectPolicy,
    /// Search/vector projection rebuild policy.
    #[serde(default)]
    pub projection: ProjectionPolicy,
}

impl TopologyContract {
    /// Parse and validate VON text.
    pub fn parse(text: &str) -> Result<Self, TopologyError> {
        let topo: Self = von::from_str(text).map_err(TopologyError::Von)?;
        topo.validate()?;
        Ok(topo)
    }

    /// Load from disk.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, TopologyError> {
        let text = std::fs::read_to_string(path.as_ref()).map_err(TopologyError::Io)?;
        Self::parse(&text)
    }

    /// Canonical VON.
    pub fn to_von(&self) -> Result<String, TopologyError> {
        self.validate()?;
        von::to_string_indented(self).map_err(TopologyError::Von)
    }

    /// Structural validation (Phase 10-A offline gates).
    pub fn validate(&self) -> Result<(), TopologyError> {
        if self.format != TOPOLOGY_FORMAT {
            return Err(TopologyError::UnsupportedFormat(self.format.clone()));
        }
        if self.version != 1 {
            return Err(TopologyError::UnsupportedVersion(self.version));
        }
        if self.id.trim().is_empty() {
            return Err(TopologyError::Invalid(
                "topology id must not be empty".into(),
            ));
        }
        if self.topology_version < 1 {
            return Err(TopologyError::Invalid(
                "topology_version must be >= 1".into(),
            ));
        }
        if self.components.is_empty() {
            return Err(TopologyError::Invalid(
                "topology must declare at least one component".into(),
            ));
        }

        let authorities: Vec<_> = self
            .components
            .iter()
            .filter(|(_, c)| c.role.is_authority())
            .map(|(id, _)| id.as_str())
            .collect();
        if authorities.is_empty() {
            return Err(TopologyError::Invalid(
                "topology must declare exactly one Authority component".into(),
            ));
        }
        if authorities.len() > 1 {
            return Err(TopologyError::Invalid(format!(
                "topology must declare exactly one Authority; found {}",
                authorities.join(", ")
            )));
        }
        let authority_id = authorities[0];

        for (name, binding) in &self.tables {
            let Some(comp) = self.components.get(&binding.authority) else {
                return Err(TopologyError::Invalid(format!(
                    "table `{name}` authority `{}` is not a component",
                    binding.authority
                )));
            };
            if !comp.role.is_authority() {
                return Err(TopologyError::Invalid(format!(
                    "table `{name}` authority `{}` does not have role=authority",
                    binding.authority
                )));
            }
            if binding.authority != authority_id {
                return Err(TopologyError::Invalid(format!(
                    "table `{name}` authority `{}` differs from sole Authority `{authority_id}` \
                     (Phase 10-A forbids multi-authority composites)",
                    binding.authority
                )));
            }
        }

        for (access_key, rule) in &self.routes {
            if let Some(pref) = &rule.preferred_component
                && !self.components.contains_key(pref)
            {
                return Err(TopologyError::Invalid(format!(
                    "route `{access_key}` preferred_component `{pref}` unknown"
                )));
            }
            if let ConsistencyIntent::ProjectionRequired { component } = &rule.default_intent
                && !self.components.contains_key(component)
            {
                return Err(TopologyError::Invalid(format!(
                    "route `{access_key}` ProjectionRequired component `{component}` unknown"
                )));
            }
        }

        for (id, c) in &self.components {
            if c.adapter.trim().is_empty() {
                return Err(TopologyError::Invalid(format!(
                    "component `{id}` adapter must not be empty"
                )));
            }
        }

        Ok(())
    }

    /// Sole authority component id (after validate).
    pub fn authority_id(&self) -> Result<&str, TopologyError> {
        self.validate()?;
        Ok(self
            .components
            .iter()
            .find(|(_, c)| c.role.is_authority())
            .map(|(id, _)| id.as_str())
            .expect("validated"))
    }

    /// Resolve table authority (or topology sole authority when table unbound).
    pub fn authority_for_table(&self, table: Option<&str>) -> Result<&str, TopologyError> {
        let sole = self.authority_id()?;
        if let Some(t) = table
            && let Some(b) = self.tables.get(t)
        {
            return Ok(b.authority.as_str());
        }
        Ok(sole)
    }

    /// Offline composite plan for an access kind + consistency intent.
    ///
    /// Does not open connections or emit private middleware commands.
    pub fn plan(
        &self,
        access: AccessKind,
        consistency: ConsistencyIntent,
        table: Option<&str>,
    ) -> Result<CompositePlan, TopologyError> {
        self.validate()?;
        let authority_id = self.authority_for_table(table)?.to_string();
        let access_key = access_route_key(access);
        let rule = self.routes.get(access_key);

        if let ConsistencyIntent::ProjectionRequired { component } = &consistency {
            let component = component.clone();
            let Some(comp) = self.components.get(&component) else {
                return Ok(CompositePlan::rejected(
                    &self.id,
                    self.topology_version,
                    authority_id,
                    access,
                    consistency,
                    format!("ProjectionRequired component `{component}` not in topology"),
                ));
            };
            if matches!(access, AccessKind::Search)
                && !matches!(comp.role, ComponentRole::SearchProjection)
            {
                return Ok(CompositePlan::rejected(
                    &self.id,
                    self.topology_version,
                    authority_id,
                    access,
                    consistency,
                    format!(
                        "ProjectionRequired `{component}` role {:?} cannot serve search",
                        comp.role
                    ),
                ));
            }
            if matches!(access, AccessKind::VectorNearest)
                && !matches!(comp.role, ComponentRole::VectorProjection)
            {
                return Ok(CompositePlan::rejected(
                    &self.id,
                    self.topology_version,
                    authority_id,
                    access,
                    consistency,
                    format!(
                        "ProjectionRequired `{component}` role {:?} cannot serve vector",
                        comp.role
                    ),
                ));
            }
        }

        match access {
            AccessKind::Write => Ok(self.plan_write(authority_id, consistency)),
            AccessKind::IdentityRead => {
                Ok(self.plan_identity_read(authority_id, consistency, rule))
            }
            AccessKind::FilteredQuery => Ok(self.plan_authority_read(
                authority_id,
                access,
                consistency,
                "filtered query has no proven projection coverage in Phase 10-A; authority only",
            )),
            AccessKind::Search => Ok(self.plan_search_or_vector(
                authority_id,
                access,
                consistency,
                ComponentRole::SearchProjection,
                rule,
            )),
            AccessKind::VectorNearest => Ok(self.plan_search_or_vector(
                authority_id,
                access,
                consistency,
                ComponentRole::VectorProjection,
                rule,
            )),
            AccessKind::BytesRange => Ok(self.plan_bytes_range(authority_id, consistency)),
            AccessKind::Effect => Ok(self.plan_effect(authority_id, consistency)),
        }
    }

    fn plan_write(&self, authority_id: String, consistency: ConsistencyIntent) -> CompositePlan {
        let outbox = self
            .components
            .iter()
            .find(|(_, c)| c.role == ComponentRole::Outbox)
            .map(|(id, _)| id.clone());
        let object_store = self
            .components
            .iter()
            .find(|(_, c)| c.role == ComponentRole::ObjectStore)
            .map(|(id, _)| id.clone());

        let mut steps = Vec::new();
        if let Some(ref obj) = object_store {
            // Bytes land pending/verified before authority publishes a committed reference.
            steps.push(CompositeStep::ObjectStep {
                component: obj.clone(),
                action: "pending".into(),
            });
            steps.push(CompositeStep::ObjectStep {
                component: obj.clone(),
                action: "write".into(),
            });
            steps.push(CompositeStep::ObjectStep {
                component: obj.clone(),
                action: "verify".into(),
            });
        }
        steps.push(CompositeStep::AuthorityStep {
            component: authority_id.clone(),
            append_outbox: outbox.is_some(),
        });
        if let Some(ref obj) = object_store {
            steps.push(CompositeStep::ObjectStep {
                component: obj.clone(),
                action: "finalize".into(),
            });
        }
        if let Some(ob) = outbox {
            steps.push(CompositeStep::EffectStep { component: ob });
        }

        let proof = if object_store.is_some() {
            "write: object pending?verify then authority; finalize publishes committed reference (not sync dual-write)"
        } else {
            "write: authority transaction; outbox append when declared (not sync dual-write)"
        };
        let mut budget_notes = Vec::new();
        if let Some(ttl) = self.object.orphan_ttl_secs {
            budget_notes.push(format!("object_orphan_ttl_secs={ttl}"));
        }

        CompositePlan {
            format: COMPOSITE_PLAN_FORMAT.into(),
            version: 1,
            topology_id: self.id.clone(),
            topology_version: self.topology_version,
            authority_id,
            access: AccessKind::Write,
            consistency,
            steps,
            proof: RouteProof::note(proof, true),
            required_watermarks: BTreeMap::new(),
            budget_notes,
            rejected: false,
            rejection: None,
        }
    }

    fn plan_identity_read(
        &self,
        authority_id: String,
        consistency: ConsistencyIntent,
        rule: Option<&RouteRule>,
    ) -> CompositePlan {
        match &consistency {
            ConsistencyIntent::Authoritative => self.plan_authority_read(
                authority_id,
                AccessKind::IdentityRead,
                consistency,
                "Authoritative identity read -> authority",
            ),
            ConsistencyIntent::ReadYourWrites
            | ConsistencyIntent::Eventual
            | ConsistencyIntent::BoundedStale { .. } => {
                let cache = self.pick_cache(rule);
                if let Some(cache_id) = cache {
                    let mut steps = vec![
                        CompositeStep::DerivedReadStep {
                            component: cache_id.clone(),
                            required_watermark: Some("authority_commit_token".into()),
                        },
                        CompositeStep::FallbackStep {
                            reason: "cache miss/stale/unavailable -> authority".into(),
                            steps: vec![CompositeStep::AuthorityStep {
                                component: authority_id.clone(),
                                append_outbox: false,
                            }],
                        },
                    ];
                    let proof_note = match &consistency {
                        ConsistencyIntent::ReadYourWrites => {
                            steps.insert(
                                0,
                                CompositeStep::FenceStep {
                                    fence: "session_fence".into(),
                                },
                            );
                            "ReadYourWrites: cache only when AppliedWatermark covers session fence; else authority"
                        }
                        ConsistencyIntent::BoundedStale { .. } => {
                            steps.insert(
                                0,
                                CompositeStep::FenceStep {
                                    fence: "bounded_stale_watermark".into(),
                                },
                            );
                            "BoundedStale: cache only when watermark proves lag within bound; else authority"
                        }
                        _ => "Eventual identity read via cache when reachable; else authority",
                    };
                    let mut watermarks = BTreeMap::new();
                    watermarks.insert(cache_id, "authority_commit_token".into());
                    CompositePlan {
                        format: COMPOSITE_PLAN_FORMAT.into(),
                        version: 1,
                        topology_id: self.id.clone(),
                        topology_version: self.topology_version,
                        authority_id,
                        access: AccessKind::IdentityRead,
                        consistency,
                        steps,
                        proof: RouteProof::note(proof_note, false),
                        required_watermarks: watermarks,
                        budget_notes: self
                            .cache
                            .stampede_budget
                            .map(|b| format!("stampede_budget={b}"))
                            .into_iter()
                            .collect(),
                        rejected: false,
                        rejection: None,
                    }
                } else {
                    self.plan_authority_read(
                        authority_id,
                        AccessKind::IdentityRead,
                        consistency,
                        "no cache component; identity read -> authority",
                    )
                }
            }
            ConsistencyIntent::ProjectionRequired { component } => {
                let component = component.clone();
                let Some(comp) = self.components.get(&component) else {
                    return CompositePlan::rejected(
                        &self.id,
                        self.topology_version,
                        authority_id,
                        AccessKind::IdentityRead,
                        consistency,
                        format!("unknown component `{component}`"),
                    );
                };
                if comp.role != ComponentRole::Cache && !comp.role.is_derived_reader() {
                    return CompositePlan::rejected(
                        &self.id,
                        self.topology_version,
                        authority_id,
                        AccessKind::IdentityRead,
                        consistency,
                        format!("component `{component}` cannot serve identity read"),
                    );
                }
                CompositePlan {
                    format: COMPOSITE_PLAN_FORMAT.into(),
                    version: 1,
                    topology_id: self.id.clone(),
                    topology_version: self.topology_version,
                    authority_id,
                    access: AccessKind::IdentityRead,
                    consistency,
                    steps: vec![CompositeStep::DerivedReadStep {
                        component,
                        required_watermark: None,
                    }],
                    proof: RouteProof::note(
                        "ProjectionRequired identity read; fail if component unhealthy (live later)",
                        false,
                    ),
                    required_watermarks: BTreeMap::new(),
                    budget_notes: Vec::new(),
                    rejected: false,
                    rejection: None,
                }
            }
        }
    }

    fn plan_authority_read(
        &self,
        authority_id: String,
        access: AccessKind,
        consistency: ConsistencyIntent,
        proof: &str,
    ) -> CompositePlan {
        CompositePlan {
            format: COMPOSITE_PLAN_FORMAT.into(),
            version: 1,
            topology_id: self.id.clone(),
            topology_version: self.topology_version,
            authority_id: authority_id.clone(),
            access,
            consistency,
            steps: vec![CompositeStep::AuthorityStep {
                component: authority_id,
                append_outbox: false,
            }],
            proof: RouteProof::note(proof, true),
            required_watermarks: BTreeMap::new(),
            budget_notes: Vec::new(),
            rejected: false,
            rejection: None,
        }
    }

    fn plan_search_or_vector(
        &self,
        authority_id: String,
        access: AccessKind,
        consistency: ConsistencyIntent,
        need_role: ComponentRole,
        rule: Option<&RouteRule>,
    ) -> CompositePlan {
        let proj = rule
            .and_then(|r| r.preferred_component.clone())
            .or_else(|| {
                self.components
                    .iter()
                    .find(|(_, c)| c.role == need_role)
                    .map(|(id, _)| id.clone())
            });
        let Some(proj_id) = proj else {
            return match rule
                .map(|r| r.fallback)
                .unwrap_or(FallbackPolicy::FailClosed)
            {
                FallbackPolicy::Authority | FallbackPolicy::FailClosed => CompositePlan::rejected(
                    &self.id,
                    self.topology_version,
                    authority_id,
                    access,
                    consistency,
                    format!(
                        "no component with role {need_role:?}; refuse approximate search/vector \
                         results from authority"
                    ),
                ),
            };
        };
        let mut watermarks = BTreeMap::new();
        watermarks.insert(proj_id.clone(), "authority_commit_token".into());
        let mut budget_notes = Vec::new();
        if let Some(lag) = self.projection.max_lag_secs {
            budget_notes.push(format!("projection_max_lag_secs={lag}"));
        }
        if !self.projection.covered_fields.is_empty() {
            budget_notes.push(format!(
                "covered_fields={}",
                self.projection.covered_fields.join(",")
            ));
        }
        budget_notes.push("candidates_only; hydrate_required".into());
        CompositePlan {
            format: COMPOSITE_PLAN_FORMAT.into(),
            version: 1,
            topology_id: self.id.clone(),
            topology_version: self.topology_version,
            authority_id: authority_id.clone(),
            access,
            consistency,
            steps: vec![
                CompositeStep::DerivedReadStep {
                    component: proj_id,
                    required_watermark: Some("authority_commit_token".into()),
                },
                CompositeStep::HydrateStep {
                    component: authority_id,
                },
                CompositeStep::CompletenessCheck {
                    policy: "drop deleted/stale candidates; no partial by default".into(),
                },
            ],
            proof: RouteProof::note(
                "search/vector -> candidate identities -> authority hydrate/validate (no fake results)",
                false,
            ),
            required_watermarks: watermarks,
            budget_notes,
            rejected: false,
            rejection: None,
        }
    }

    fn plan_bytes_range(
        &self,
        authority_id: String,
        consistency: ConsistencyIntent,
    ) -> CompositePlan {
        let obj = self
            .components
            .iter()
            .find(|(_, c)| c.role == ComponentRole::ObjectStore)
            .map(|(id, _)| id.clone());
        let Some(obj_id) = obj else {
            return CompositePlan::rejected(
                &self.id,
                self.topology_version,
                authority_id,
                AccessKind::BytesRange,
                consistency,
                "no ObjectStore component for bytes range",
            );
        };
        CompositePlan {
            format: COMPOSITE_PLAN_FORMAT.into(),
            version: 1,
            topology_id: self.id.clone(),
            topology_version: self.topology_version,
            authority_id: authority_id.clone(),
            access: AccessKind::BytesRange,
            consistency,
            steps: vec![
                CompositeStep::AuthorityStep {
                    component: authority_id,
                    append_outbox: false,
                },
                CompositeStep::ObjectStep {
                    component: obj_id,
                    action: "range_read".into(),
                },
            ],
            proof: RouteProof::note(
                "bytes range: authority object metadata then object-store range_read",
                true,
            ),
            required_watermarks: BTreeMap::new(),
            budget_notes: Vec::new(),
            rejected: false,
            rejection: None,
        }
    }

    fn plan_effect(&self, authority_id: String, consistency: ConsistencyIntent) -> CompositePlan {
        let outbox = self
            .components
            .iter()
            .find(|(_, c)| c.role == ComponentRole::Outbox)
            .map(|(id, _)| id.clone());
        let Some(ob) = outbox else {
            return CompositePlan::rejected(
                &self.id,
                self.topology_version,
                authority_id,
                AccessKind::Effect,
                consistency,
                "no Outbox component for effects",
            );
        };
        CompositePlan {
            format: COMPOSITE_PLAN_FORMAT.into(),
            version: 1,
            topology_id: self.id.clone(),
            topology_version: self.topology_version,
            authority_id,
            access: AccessKind::Effect,
            consistency,
            steps: vec![CompositeStep::EffectStep { component: ob }],
            proof: RouteProof::note("effect: outbox-driven idempotent consumer", true),
            required_watermarks: BTreeMap::new(),
            budget_notes: Vec::new(),
            rejected: false,
            rejection: None,
        }
    }

    fn pick_cache(&self, rule: Option<&RouteRule>) -> Option<String> {
        if let Some(pref) = rule.and_then(|r| r.preferred_component.as_ref())
            && self
                .components
                .get(pref)
                .is_some_and(|c| c.role == ComponentRole::Cache)
        {
            return Some(pref.clone());
        }
        self.components
            .iter()
            .find(|(_, c)| c.role == ComponentRole::Cache)
            .map(|(id, _)| id.clone())
    }
}

fn access_route_key(access: AccessKind) -> &'static str {
    match access {
        AccessKind::IdentityRead => "identity_read",
        AccessKind::FilteredQuery => "filtered_query",
        AccessKind::Search => "search",
        AccessKind::VectorNearest => "vector_nearest",
        AccessKind::Write => "write",
        AccessKind::BytesRange => "bytes_range",
        AccessKind::Effect => "effect",
    }
}

/// Verify helper: collect diagnostic strings.
pub fn verify_report(topo: &TopologyContract) -> Result<Vec<String>, TopologyError> {
    topo.validate()?;
    let mut notes = Vec::new();
    notes.push(format!(
        "ok: topology `{}` v{} format={}",
        topo.id, topo.topology_version, topo.format
    ));
    let auth = topo.authority_id()?;
    notes.push(format!("authority: {auth}"));
    let roles: BTreeSet<_> = topo
        .components
        .values()
        .map(|c| format!("{:?}", c.role))
        .collect();
    notes.push(format!(
        "roles: {}",
        roles.into_iter().collect::<Vec<_>>().join(", ")
    ));
    if !topo
        .components
        .values()
        .any(|c| c.role == ComponentRole::Outbox)
    {
        notes.push(
            "warning: no Outbox component ? writes will not declare durable propagation duty"
                .into(),
        );
    }
    if !topo
        .components
        .values()
        .any(|c| c.role == ComponentRole::Cache)
    {
        notes.push(
            "note: no Cache component ? BoundedStale/Eventual identity reads use authority".into(),
        );
    }
    Ok(notes)
}

/// Topology errors.
#[derive(Debug)]
pub enum TopologyError {
    /// I/O.
    Io(std::io::Error),
    /// VON.
    Von(von::VonError),
    /// Bad format discriminator.
    UnsupportedFormat(String),
    /// Bad version.
    UnsupportedVersion(i64),
    /// Invalid topology.
    Invalid(String),
}

impl std::fmt::Display for TopologyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Von(e) => write!(f, "{e}"),
            Self::UnsupportedFormat(s) => write!(f, "unsupported topology format `{s}`"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported topology version {v}"),
            Self::Invalid(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for TopologyError {}
