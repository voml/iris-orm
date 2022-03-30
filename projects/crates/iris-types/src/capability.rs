//! Capability negotiation model.

use serde::{Deserialize, Serialize};

/// Query pushdown capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryCaps {
    /// Equality / inequality filters on fields.
    pub filter_cmp: bool,
    /// Boolean field predicates.
    pub filter_bool: bool,
    /// Logical `&&` / `||`.
    pub filter_logic: bool,
    /// Field sort.
    pub sort: bool,
    /// Skip/take pagination.
    pub page: bool,
    /// Field projection / rename.
    pub project: bool,
}

impl QueryCaps {
    /// Everything supported (reference adapter).
    pub fn full() -> Self {
        Self {
            filter_cmp: true,
            filter_bool: true,
            filter_logic: true,
            sort: true,
            page: true,
            project: true,
        }
    }

    /// Read-only scans without filter (forces reject on filter).
    pub fn scan_only() -> Self {
        Self {
            filter_cmp: false,
            filter_bool: false,
            filter_logic: false,
            sort: false,
            page: false,
            project: false,
        }
    }
}

/// Write capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteCaps {
    /// Insert rows.
    pub insert: bool,
    /// Update / patch.
    pub update: bool,
    /// Delete.
    pub delete: bool,
}

impl WriteCaps {
    /// No writes.
    pub fn none() -> Self {
        Self {
            insert: false,
            update: false,
            delete: false,
        }
    }

    /// Full writes.
    pub fn full() -> Self {
        Self {
            insert: true,
            update: true,
            delete: true,
        }
    }
}

/// Compensation budgets (Phase 1: recorded, enforced on compensated paths later).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompensationBudget {
    /// Max rows pulled for compensation.
    pub max_rows: u64,
    /// Max bytes.
    pub max_bytes: u64,
    /// Max backend round-trips.
    pub max_round_trips: u64,
    /// Max wall time in milliseconds.
    pub max_millis: u64,
    /// Max memory bytes.
    pub max_memory_bytes: u64,
    /// Max association fan-out.
    pub max_assoc_fanout: u64,
}

impl Default for CompensationBudget {
    fn default() -> Self {
        Self {
            max_rows: 10_000,
            max_bytes: 16 * 1024 * 1024,
            max_round_trips: 32,
            max_millis: 5_000,
            max_memory_bytes: 64 * 1024 * 1024,
            max_assoc_fanout: 64,
        }
    }
}

impl CompensationBudget {
    /// Fail when a result set would exceed the row budget.
    pub fn enforce_rows(&self, rows: u64) -> Result<(), String> {
        if rows > self.max_rows {
            Err(format!(
                "compensation budget exceeded: {rows} rows > max_rows {}",
                self.max_rows
            ))
        } else {
            Ok(())
        }
    }

    /// Fail when estimated bytes would exceed the byte budget.
    pub fn enforce_bytes(&self, bytes: u64) -> Result<(), String> {
        if bytes > self.max_bytes {
            Err(format!(
                "compensation budget exceeded: {bytes} bytes > max_bytes {}",
                self.max_bytes
            ))
        } else {
            Ok(())
        }
    }

    /// Fail when round-trips would exceed the budget.
    pub fn enforce_round_trips(&self, trips: u64) -> Result<(), String> {
        if trips > self.max_round_trips {
            Err(format!(
                "compensation budget exceeded: {trips} round-trips > max_round_trips {}",
                self.max_round_trips
            ))
        } else {
            Ok(())
        }
    }
}

/// Versioned capability set returned by a datasource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitySet {
    /// Backend / adapter id (`reference`, `yydb`, ...).
    pub backend_id: String,
    /// Backend version label.
    pub backend_version: String,
    /// Maximum Iris IR version this backend accepts.
    pub ir_version_max: iris_ir::IrVersion,
    /// Query capabilities.
    pub query: QueryCaps,
    /// Write capabilities.
    pub write: WriteCaps,
    /// Compensation budgets.
    pub budget: CompensationBudget,
}

impl Default for CapabilitySet {
    fn default() -> Self {
        Self::reference_full()
    }
}

impl CapabilitySet {
    /// In-memory reference adapter with full Phase 1 query support.
    pub fn reference_full() -> Self {
        Self {
            backend_id: "reference".into(),
            backend_version: "0.1.0".into(),
            ir_version_max: iris_ir::IrVersion::PHASE1,
            query: QueryCaps::full(),
            write: WriteCaps::none(),
            budget: CompensationBudget::default(),
        }
    }

    /// Capability id list required by a physical op.
    pub fn required_for(op: &iris_ir::PhysicalOp) -> Vec<String> {
        use iris_ir::PhysicalOp;
        match op {
            PhysicalOp::Scan { .. } => vec!["scan".into()],
            PhysicalOp::Filter { .. } => vec!["filter".into()],
            PhysicalOp::Project { .. } => vec!["project".into()],
            PhysicalOp::Sort { .. } => vec!["sort".into()],
            PhysicalOp::Skip { .. } | PhysicalOp::Take { .. } => vec!["page".into()],
            PhysicalOp::Collect => vec!["collect".into()],
        }
    }
}
