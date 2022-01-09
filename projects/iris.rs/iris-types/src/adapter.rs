//! Foreign-store adapter SPI (host-neutral shapes; no backend command text).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::value::Value;

/// One observed column from a foreign catalog inspect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedColumn {
    /// Physical column name.
    pub name: String,
    /// Backend type name as reported (e.g. `INTEGER`, `TEXT`).
    pub type_name: String,
    /// Nullable when true.
    pub nullable: bool,
    /// Part of primary key when true.
    pub primary_key: bool,
}

/// One observed table/relation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedTable {
    /// Physical table name.
    pub name: String,
    /// Columns in ordinal order.
    pub columns: Vec<ObservedColumn>,
}

/// Read-only catalog snapshot from `inspect`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedCatalog {
    /// Backend id (`sqlite`, ...).
    pub backend_id: String,
    /// Tables found.
    pub tables: Vec<ObservedTable>,
}

impl ObservedCatalog {
    /// Lookup by physical table name.
    pub fn table(&self, name: &str) -> Option<&ObservedTable> {
        self.tables.iter().find(|t| t.name == name)
    }
}

/// How a VOS field maps onto a physical column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldMapping {
    /// VOS field name.
    pub vos_field: String,
    /// Physical column name.
    pub physical_column: String,
    /// Declared VOS type label.
    pub vos_type: String,
    /// Physical type label.
    pub physical_type: String,
    /// Mapping quality.
    pub quality: MappingQuality,
    /// Human note / waiver reason.
    pub note: Option<String>,
}

/// Mapping fidelity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MappingQuality {
    /// Exact round-trip expected.
    Exact,
    /// Explicit compatible encoding.
    Compatible,
    /// Blocked unless waived.
    LossyBlocked,
}

/// Table-level adopt / push mapping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableMapping {
    /// VOS table name.
    pub vos_table: String,
    /// Physical table name.
    pub physical_table: String,
    /// Field mappings.
    pub fields: Vec<FieldMapping>,
    /// Blocking diagnostics (empty when adoptable).
    pub blockers: Vec<String>,
}

/// Reviewable mapping manifest (VON/JSON serializable).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MappingManifest {
    /// Adapter id.
    pub adapter_id: String,
    /// Adapter version label.
    pub adapter_version: String,
    /// Tables.
    pub tables: Vec<TableMapping>,
}

/// One logical migration step (non-SQL).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogicalChange {
    /// Create a table from VOS definition.
    CreateTable {
        /// VOS table name.
        vos_table: String,
    },
    /// Add a column.
    AddField {
        /// VOS table.
        vos_table: String,
        /// VOS field.
        vos_field: String,
    },
}

/// Versioned logical migration plan (hashable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogicalMigrationPlan {
    /// Plan id.
    pub id: String,
    /// Parent schema fingerprint.
    pub parent_fingerprint: String,
    /// Target schema fingerprint.
    pub target_fingerprint: String,
    /// Ordered logical changes.
    pub changes: Vec<LogicalChange>,
    /// Destructive when true (requires explicit apply).
    pub destructive: bool,
}

/// Drift between local VOS contract and observed catalog.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriftReport {
    /// Tables only in VOS.
    pub missing_physical_tables: Vec<String>,
    /// Tables only in catalog.
    pub extra_physical_tables: Vec<String>,
    /// Column-level mismatches.
    pub field_mismatches: Vec<String>,
}

impl DriftReport {
    /// True when VOS-declared tables/columns match the catalog (ignores unrelated physical tables).
    pub fn is_push_satisfied(&self) -> bool {
        self.missing_physical_tables.is_empty() && self.field_mismatches.is_empty()
    }

    /// True when the catalog matches VOS exactly (no extra physical tables).
    pub fn is_clean(&self) -> bool {
        self.is_push_satisfied() && self.extra_physical_tables.is_empty()
    }
}

/// Parameterized row write used by adapters (values stay typed; no SQL here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowWrite {
    /// Logical / mapped table name.
    pub table: String,
    /// Primary key field name.
    pub primary_key: String,
    /// Full row for insert, or key+patch fields for update.
    pub fields: BTreeMap<String, Value>,
}
