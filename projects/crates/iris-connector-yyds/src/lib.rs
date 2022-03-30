//! Native YYDS connector --?Phase 5 readiness gate.
//!
//! Iris will speak **formal VOS / shared VOS IR** to YYDS only. Historical SQL
//! gateways (`yyds-gateway/src/sql`, ODBC, we-trust-sqlite/mysql/postgres, --?
//! are forbidden product paths for this connector.
//!
//! As of 2026-08-12 the sibling `yyds` tree still exposes OpsGraph / DsValue
//! clients rather than a versioned VOS IR executor + `.yyds`/`.yykv` lifecycle
//! facade Iris can consume safely. Until that ships, [`YydsSource::connect`]
//! returns [`Error::NotReady`] and capability handshake reports unavailable.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use iris_ir::IrVersion;
use iris_types::{CapabilitySet, CompensationBudget, QueryCaps, WriteCaps};
use serde::{Deserialize, Serialize};

/// Connector identifier.
pub const BACKEND_ID: &str = "yyds";

/// Adapter / connector crate version label.
pub const CONNECTOR_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Stable readiness code for tooling and diagnostics.
pub const READINESS_CODE: &str = "IRIS-YYDS-VOS-NOT-READY";

/// Forbidden legacy surfaces that must never be wired through this connector.
pub const FORBIDDEN_LEGACY_SURFACES: &[&str] = &[
    "yyds-gateway/src/sql",
    "oak-sql",
    "yyds-odbc",
    "we-trust-sqlite",
    "we-trust-mysql",
    "we-trust-postgres",
    "we-trust-sqlserver",
    "query_with_sql",
    "SqlQuery",
];

/// What Iris requires from YYDS before enabling the native connector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessReport {
    /// Connector id.
    pub backend_id: String,
    /// Always false until YYDS publishes a VOS executor facade Iris can depend on.
    pub vos_executor_ready: bool,
    /// Always false until `.yyds` catalog + `.yykv` shard lifecycle is a public API.
    pub catalog_lifecycle_ready: bool,
    /// Always false until ACL / audit session context is part of the VOS protocol.
    pub control_plane_context_ready: bool,
    /// Stable diagnostic code.
    pub code: String,
    /// Human-readable blocker summary (no secrets).
    pub message: String,
    /// Surfaces that remain forbidden even after readiness.
    pub forbidden_legacy_surfaces: Vec<String>,
}

impl ReadinessReport {
    /// Probe current readiness (compile-time gate until YYDS ships the facade).
    pub fn probe() -> Self {
        Self {
            backend_id: BACKEND_ID.into(),
            vos_executor_ready: false,
            catalog_lifecycle_ready: false,
            control_plane_context_ready: false,
            code: READINESS_CODE.into(),
            message: "YYDS formal VOS IR executor / .yyds+.yykv lifecycle facade is not yet \
                      available for Iris; connector refuses connect and will not use SQL gateways"
                .into(),
            forbidden_legacy_surfaces: FORBIDDEN_LEGACY_SURFACES
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
        }
    }

    /// True only when every readiness bit is set.
    pub fn is_ready(&self) -> bool {
        self.vos_executor_ready && self.catalog_lifecycle_ready && self.control_plane_context_ready
    }
}

/// Optional session context Iris will forward once YYDS is ready.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct YydsSessionContext {
    /// Tenant / namespace id (opaque string; not a SQL schema name).
    pub tenant_id: Option<String>,
    /// Principal / subject for ACL.
    pub principal: Option<String>,
    /// Correlation id for audit.
    pub audit_id: Option<String>,
    /// Request deadline in unix millis.
    pub deadline_unix_ms: Option<u64>,
}

/// IR version handshake Iris will perform with YYDS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IrHandshake {
    /// Iris IR version offered.
    pub iris_ir_version: IrVersion,
    /// Maximum IR version YYDS claims (unknown until ready).
    pub yyds_ir_version_max: Option<IrVersion>,
    /// Negotiated version when ready.
    pub negotiated: Option<IrVersion>,
}

/// Placeholder native YYDS datasource. Construction always fails until readiness clears.
#[derive(Debug)]
pub struct YydsSource {
    _private: (),
}

impl YydsSource {
    /// Intended capability advertisement once YYDS VOS is live.
    ///
    /// Today this describes the **target** surface; [`connect`] still fails.
    pub fn target_capabilities() -> CapabilitySet {
        CapabilitySet {
            backend_id: BACKEND_ID.into(),
            backend_version: CONNECTOR_VERSION.into(),
            ir_version_max: IrVersion::PHASE1,
            query: QueryCaps::full(),
            write: WriteCaps::full(),
            budget: CompensationBudget::default(),
        }
    }

    /// Current readiness probe.
    pub fn readiness() -> ReadinessReport {
        ReadinessReport::probe()
    }

    /// Connect to a YYDS endpoint.
    ///
    /// Always returns [`Error::NotReady`] until the readiness report is green.
    /// Never opens historical SQL gateways as a fallback.
    pub fn connect(_endpoint: &str, _ctx: YydsSessionContext) -> Result<Self> {
        let report = Self::readiness();
        if !report.is_ready() {
            return Err(Error::NotReady(report));
        }
        // Future: open formal VOS protocol client here.
        Err(Error::Policy(
            "readiness bits set but VOS client binding is not implemented yet".into(),
        ))
    }

    /// Reject any attempt to attach a legacy SQL surface by name.
    pub fn reject_legacy_surface(name: &str) -> Result<()> {
        let lowered = name.to_ascii_lowercase();
        for banned in FORBIDDEN_LEGACY_SURFACES {
            if lowered.contains(&banned.to_ascii_lowercase()) {
                return Err(Error::ForbiddenLegacy(format!(
                    "refusing legacy YYDS surface `{name}` (matched `{banned}`)"
                )));
            }
        }
        Ok(())
    }
}

/// Connector errors.
#[derive(Debug)]
pub enum Error {
    /// YYDS VOS product surface is not ready for Iris.
    NotReady(ReadinessReport),
    /// Caller attempted a forbidden SQL / legacy gateway path.
    ForbiddenLegacy(String),
    /// Policy / contract violation.
    Policy(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotReady(r) => write!(f, "{}: {}", r.code, r.message),
            Self::ForbiddenLegacy(s) | Self::Policy(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for Error {}

/// Result alias.
pub type Result<T> = std::result::Result<T, Error>;
