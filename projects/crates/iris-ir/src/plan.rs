//! Physical plan container.

use serde::{Deserialize, Serialize};

use crate::envelope::IrEnvelope;
use crate::op::PhysicalOp;

/// How a plan node is realized against a datasource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RealizationClass {
    /// Backend executes the semantics exactly.
    Native,
    /// Different mechanism, observably equivalent.
    Equivalent,
    /// In-process compensation under explicit budgets.
    Compensated,
    /// Must fail before execution.
    Rejected,
}

/// One planned physical node with realization class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedNode {
    /// Physical operation.
    pub op: PhysicalOp,
    /// Realization decision.
    pub realization: RealizationClass,
    /// Human reason when rejected / compensated.
    pub note: Option<String>,
}

/// Complete physical plan + envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhysicalPlan {
    /// Envelope metadata.
    pub envelope: IrEnvelope,
    /// Ordered nodes.
    pub nodes: Vec<PlannedNode>,
}

impl PhysicalPlan {
    /// True when any node is rejected (must not execute).
    pub fn is_rejected(&self) -> bool {
        self.nodes
            .iter()
            .any(|n| n.realization == RealizationClass::Rejected)
    }

    /// First rejection note, if any.
    pub fn rejection_note(&self) -> Option<&str> {
        self.nodes.iter().find_map(|n| {
            if n.realization == RealizationClass::Rejected {
                n.note.as_deref()
            } else {
                None
            }
        })
    }
}
