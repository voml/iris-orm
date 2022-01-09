//! Iris ORM public facade.
//!
//! Applications depend on this crate only. Connector and adapter crates are
//! workspace-private and must not be re-exported as SQL or foreign-command APIs.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub use iris_ir::{
    AccessKind, AppliedWatermark, COMPOSITE_PLAN_FORMAT, CmpOp, CommitToken, CompositePlan,
    CompositeStep, ConsistencyIntent, DEFAULT_COMMIT_SHARD, EffectKind, HydrateCompleteness,
    HydrateDropReason, HydrateResult, HydratedEntity, IrEnvelope, IrVersion, ObjectError,
    ObjectHash, ObjectId, ObjectLifecycleState, ObjectMeta, ObjectReference, ObjectResult,
    OutboxAppend, OutboxEffect, OutboxRecord, PhysicalOp, PhysicalPlan, PlannedNode, Pred,
    ProjectField, ProjectionCandidate, ProjectionDocument, ProjectionGeneration, RealizationClass,
    RouteProof, SchemaFingerprint, SemanticHash, SortKey, require_transition,
};
pub use iris_types::{
    AppliedWatermarkState, AuthorityEntity, AuthorityEntityLookup, CachePolicy, CacheReadAction,
    CacheReadContext, CacheWatermarkProbe, CapabilitySet, CompensationBudget, ComponentRole,
    DatasourceConfig, DatasourceKind, Diagnostic, DriftReport, EXPLAIN_FORMAT, Error,
    ExplainReport, ExplainSafety, ExplainStep, FallbackPolicy, FieldMapping, FsObjectStore,
    GenerateConfig, GenerationState, Iris, IrisLock, IrisProject, LOCK_FILE, LiveWatermarkView,
    LocalProjectionStore, LogicalChange, LogicalMigrationPlan, MapAuthorityLookup, MappingManifest,
    MappingQuality, OBJECT_HASH_ALG_BLAKE3, OBJECT_REF_FORMAT, ObjectPolicy, ObjectStateCounts,
    ObjectStatusRow, ObjectStoreStatusReport, ObservedCatalog, ObservedColumn, ObservedTable,
    OutboxPolicy, PROJECT_FILE, PROJECT_FORMAT, PROJECTION_ALIAS_FORMAT,
    PROJECTION_GENERATION_FORMAT, PROJECTION_STATUS_FORMAT, PROJECTION_VERIFY_FORMAT,
    PhysicalExplain, Planner, ProjectError, ProjectionComponentStatus, ProjectionPolicy,
    ProjectionRebuildStatus, ProjectionStatusReport, ProjectionStoreError, ProjectionStoreResult,
    ProjectionVerifyCheck, ProjectionVerifyReport, QueryCaps, RebuildHandle, RebuildValidation,
    ReferenceStore, Result, RouteRule, Row, RowWrite, Runtime, Session, StageKind, StampedeBudget,
    StampedePermit, TOPOLOGY_ACTIVATION_FORMAT, TOPOLOGY_DIR, TOPOLOGY_FORMAT, TableBinding,
    TableMapping, TopologyActivateReport, TopologyActivation, TopologyComponent, TopologyContract,
    TopologyError, TopologyHandshake, TruthMode, Value, WriteCaps, activate_topology,
    assert_explain_safe, collect_schema_paths, decide_cache_read, expand_env, explain_from_plan,
    explain_topology, hydrate_candidates, load_activation, load_schema_document,
    physical_explain_from_plan, projection_status, projection_status_offline, read_schema,
    reader_version_accepted, resolve_path, row_from_pairs, scan_explain_text, table_name_class_hints,
    table_name_class_hints_from_source, verify_projection, verify_report, watermark_covers,
    writer_version_ok, DEFAULT_CACHE_ROOT, DEFAULT_GENERATE_DIR, DEFAULT_LOCK_PATH,
    DEFAULT_MIGRATIONS_DIR, default_migration_plan, find_workspace_root,
};

/// Library version string.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
