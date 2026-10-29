//! Iris runtime core: parse -> plan -> capability -> reference execution.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod adapter;
mod cache_route;
mod capability;
mod contract;
mod diagnostic;
mod error;
mod explain;
mod hydrate;
mod lower;
mod object_store;
mod planner;
mod project;
mod projection_status;
mod projection_store;
mod projection_verify;
mod reference;
mod schema;
mod session;
mod topology;
mod topology_activate;
mod typed_plan;
mod uuid_v7;
mod value;

pub use adapter::{
    DriftReport, FieldMapping, LogicalChange, LogicalMigrationPlan, MappingManifest, MappingQuality, ObservedCatalog, ObservedColumn,
    ObservedTable, RowWrite, TableMapping,
};

pub use cache_route::{AppliedWatermarkState, CacheReadAction, CacheReadContext, StampedeBudget, StampedePermit, decide_cache_read};
pub use capability::{CapabilitySet, CompensationBudget, QueryCaps, WriteCaps};
pub use contract::{ACCEPTED_CONTRACT_FORMAT, CODE_ENVELOPE, CODE_FINGERPRINT, CODE_PRODUCER, ContractError, IrisContract};
pub use diagnostic::{Diagnostic, StageKind};
pub use error::{Error, Result};
pub use explain::{
    EXPLAIN_FORMAT, ExplainReport, ExplainSafety, ExplainStep, PhysicalExplain, assert_explain_safe, explain_from_plan, explain_topology,
    physical_explain_from_plan, scan_explain_text,
};
pub use hydrate::{AuthorityEntity, AuthorityEntityLookup, MapAuthorityLookup, hydrate_candidates};
pub use object_store::{FsObjectStore, OBJECT_HASH_ALG_BLAKE3, OBJECT_REF_FORMAT, ObjectStateCounts, ObjectStatusRow, ObjectStoreStatusReport};
pub use planner::Planner;
pub use project::{
    DEFAULT_CACHE_ROOT, DEFAULT_GENERATE_DIR, DEFAULT_LOCK_PATH, DEFAULT_MIGRATIONS_DIR, DEFAULT_SCHEMA, DatasourceConfig, DatasourceKind,
    GenerateConfig, IrisLock, IrisProject, LOCK_FILE, PROJECT_FILE, PROJECT_FORMAT, ProjectError, TruthMode, default_migration_plan,
    expand_env, find_workspace_root, resolve_path,
};
pub use projection_status::{
    CacheWatermarkProbe, LiveWatermarkView, PROJECTION_STATUS_FORMAT, ProjectionComponentStatus, ProjectionStatusReport, projection_status,
    projection_status_offline, watermark_covers,
};
pub use projection_store::{
    GenerationState, LocalProjectionStore, PROJECTION_ALIAS_FORMAT, PROJECTION_GENERATION_FORMAT, ProjectionRebuildStatus,
    ProjectionStoreError, ProjectionStoreResult, RebuildHandle, RebuildValidation,
};
pub use projection_verify::{PROJECTION_VERIFY_FORMAT, ProjectionVerifyCheck, ProjectionVerifyReport, verify_projection};
pub use reference::{ReferenceStore, row_from_pairs};
pub use schema::{
    collect_schema_paths, load_schema_document, parse_schema, read_schema, table_name_class_hints, table_name_class_hints_from_source,
};
pub use session::{Iris, Session};
pub use topology::{
    CachePolicy, ComponentRole, FallbackPolicy, ObjectPolicy, OutboxPolicy, ProjectionPolicy, RouteRule, TOPOLOGY_DIR, TOPOLOGY_FORMAT,
    TableBinding, TopologyComponent, TopologyContract, TopologyError, verify_report,
};
pub use topology_activate::{
    TOPOLOGY_ACTIVATION_FORMAT, TopologyActivateReport, TopologyActivation, TopologyHandshake, activate_topology, load_activation,
    reader_version_accepted, writer_version_ok,
};
pub use typed_plan::{
    CODE_ARGUMENT, CODE_ARGUMENT_TYPE, CODE_CANONICAL_VALUE, CODE_FILTER, CODE_PARAMETER_TYPE, CODE_RESULT_TYPE, CODE_RESULT_UNSUPPORTED,
    FilterBinding, OperationBinding, TypedArgument, TypedColumn, TypedLogicalPlan, TypedParameter, TypedPlanError, is_date, is_decimal,
    is_time, is_utc_datetime,
};
pub use uuid_v7::{is_v7, uuid};
pub use value::{
    Row, TAGGED_JSON_KEY, Value, bytes_from_tagged_json, bytes_to_hex, hex_to_bytes, is_tagged_bytes_json, tagged_bytes_json,
};

use iris_ir::{IrVersion, PhysicalPlan, RealizationClass};

/// Process-local registry skeleton (Phase 1: single reference datasource).
#[derive(Debug, Default)]
pub struct Runtime {
    /// Default capability set used when opening the reference source.
    pub capabilities: CapabilitySet,
}

impl Runtime {
    /// Create a runtime with full reference-adapter capabilities.
    pub fn new() -> Self {
        Self { capabilities: CapabilitySet::reference_full() }
    }

    /// Open the process Iris facade bound to an in-memory reference store.
    pub fn open_reference(self, store: ReferenceStore) -> Iris {
        Iris::new(self.capabilities, store)
    }

    /// Deprecated Phase 0 helper -- returns a rejected empty plan.
    pub fn placeholder_plan(&self) -> PhysicalPlan {
        use iris_ir::{EffectKind, IrEnvelope, PhysicalOp, PlannedNode, SchemaFingerprint, SemanticHash};
        PhysicalPlan {
            envelope: IrEnvelope {
                vos_contract_version: "0".into(),
                ir_version: IrVersion::PHASE1,
                schema_fingerprint: SchemaFingerprint::unbound(),
                operation_id: "placeholder".into(),
                effect: EffectKind::Read,
                required_capabilities: Vec::new(),
                span_start: 0,
                span_end: 0,
                semantic_hash: SemanticHash(0),
            },
            nodes: vec![PlannedNode {
                op: PhysicalOp::Collect,
                realization: RealizationClass::Rejected,
                note: Some("Phase 0 placeholder".into()),
            }],
        }
    }
}
