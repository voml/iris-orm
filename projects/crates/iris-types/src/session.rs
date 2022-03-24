//! Public session / Iris process API.

use iris_ir::PhysicalPlan;

use crate::capability::CapabilitySet;
use crate::error::Result;
use crate::planner::Planner;
use crate::reference::ReferenceStore;
use crate::value::Row;

/// Process-level Iris handle bound to one reference datasource (Phase 1).
#[derive(Debug)]
pub struct Iris {
    capabilities: CapabilitySet,
    store: ReferenceStore,
}

impl Iris {
    /// Construct from capabilities + store.
    pub fn new(capabilities: CapabilitySet, store: ReferenceStore) -> Self {
        Self {
            capabilities,
            store,
        }
    }

    /// Open a session on the reference datasource.
    pub fn session(&self) -> Session<'_> {
        Session {
            planner: Planner::new(self.capabilities.clone()),
            store: &self.store,
        }
    }

    /// Shared store (tests / seeding).
    pub fn store(&self) -> &ReferenceStore {
        &self.store
    }

    /// Mutable store for seeding.
    pub fn store_mut(&mut self) -> &mut ReferenceStore {
        &mut self.store
    }
}

/// Session boundary for executing VOS against the bound datasource.
///
/// Public escape hatch names align with the TS generated client:
/// - [`Session::query`] ↔ `db.$query` (DML, returns rows)
/// - [`Session::execute`] ↔ `db.$execute` (DDL / unit-valued VOS)
#[derive(Debug)]
pub struct Session<'a> {
    planner: Planner,
    store: &'a ReferenceStore,
}

impl Session<'_> {
    /// Parse + plan a VOS source (fails before execute on unsupported ops).
    pub fn plan(&self, source: &str) -> Result<PhysicalPlan> {
        self.planner.plan_source(source)
    }

    /// Plan then execute DML on the reference adapter (enforces compensation row budget).
    ///
    /// Counterpart of generated `db.$query(vosText, parameters?)`.
    pub fn query(&self, source: &str) -> Result<Vec<Row>> {
        let plan = self.plan(source)?;
        self.store
            .execute_plan_with_budget(&plan, Some(&self.planner.capabilities.budget))
    }

    /// Execute unit-valued / DDL-shaped VOS and map success to `()`.
    ///
    /// Counterpart of generated `db.$execute(vosText, parameters?)`.
    /// Phase 1 reference path still plans through the same pipeline; write/DDL
    /// ops may be rejected by capability or lower until a write-capable backend
    /// is bound.
    pub fn execute(&self, source: &str) -> Result<()> {
        let _rows = self.query(source)?;
        Ok(())
    }

    /// Reference interpreter path (no capability gating) for conformance.
    pub fn interpret(&self, source: &str) -> Result<Vec<Row>> {
        self.store.interpret_source(source)
    }

    /// Parse + plan a VOS source.
    #[deprecated(note = "renamed to Session::plan")]
    pub fn plan_vos(&self, source: &str) -> Result<PhysicalPlan> {
        self.plan(source)
    }

    /// Plan then execute DML (returns rows).
    #[deprecated(note = "renamed to Session::query (aligns with db.$query)")]
    pub fn execute_vos(&self, source: &str) -> Result<Vec<Row>> {
        self.query(source)
    }

    /// Reference interpreter path for conformance.
    #[deprecated(note = "renamed to Session::interpret")]
    pub fn interpret_vos(&self, source: &str) -> Result<Vec<Row>> {
        self.interpret(source)
    }
}
