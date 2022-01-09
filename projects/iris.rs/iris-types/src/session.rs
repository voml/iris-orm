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
#[derive(Debug)]
pub struct Session<'a> {
    planner: Planner,
    store: &'a ReferenceStore,
}

impl Session<'_> {
    /// Parse + plan a VOS source (fails before execute on unsupported ops).
    pub fn plan_vos(&self, source: &str) -> Result<PhysicalPlan> {
        self.planner.plan_source(source)
    }

    /// Plan then execute on the reference adapter (enforces compensation row budget).
    pub fn execute_vos(&self, source: &str) -> Result<Vec<Row>> {
        let plan = self.plan_vos(source)?;
        self.store
            .execute_plan_with_budget(&plan, Some(&self.planner.capabilities.budget))
    }

    /// Reference interpreter path (no capability gating) for conformance.
    pub fn interpret_vos(&self, source: &str) -> Result<Vec<Row>> {
        self.store.interpret_source(source)
    }
}
