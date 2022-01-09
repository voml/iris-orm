//! Plan + capability negotiation.

use iris_ir::{
    EffectKind, IrEnvelope, IrVersion, PhysicalOp, PhysicalPlan, PlannedNode, RealizationClass,
    SchemaFingerprint, hash_ops,
};
use vos::ast::Span;

use crate::capability::CapabilitySet;
use crate::diagnostic::Diagnostic;
use crate::error::{Error, Result};
use crate::lower;

/// Planner: parse-ready program -> physical plan with realization classes.
#[derive(Debug, Clone)]
pub struct Planner {
    /// Capability set for the target datasource.
    pub capabilities: CapabilitySet,
}

impl Planner {
    /// Create a planner for the given capabilities.
    pub fn new(capabilities: CapabilitySet) -> Self {
        Self { capabilities }
    }

    /// Plan a VOS source string.
    pub fn plan_source(&self, source: &str) -> Result<PhysicalPlan> {
        let program = vos::parse_program(source).map_err(|diags| {
            let first = diags
                .errors
                .first()
                .map(Diagnostic::from_vos_parse)
                .unwrap_or_else(|| Diagnostic::plan_rejected("parse failed", Span::empty(0), None));
            Error::diagnostic(first)
        })?;
        self.plan_program(&program)
    }

    /// Plan an already-parsed program.
    pub fn plan_program(&self, program: &vos::ast::expr::Program) -> Result<PhysicalPlan> {
        let (ops, span) = lower::lower_program(program)?;
        self.plan_ops(ops, span)
    }

    /// Assign realization classes and stamp the envelope.
    pub fn plan_ops(&self, ops: Vec<PhysicalOp>, span: Span) -> Result<PhysicalPlan> {
        let mut required = Vec::new();
        let mut nodes = Vec::with_capacity(ops.len());
        for op in ops {
            for cap in CapabilitySet::required_for(&op) {
                if !required.contains(&cap) {
                    required.push(cap);
                }
            }
            let (realization, note) = self.classify(&op);
            if realization == RealizationClass::Rejected {
                let message = note
                    .clone()
                    .unwrap_or_else(|| "operation rejected by capability policy".into());
                // Still build a rejected plan node, but fail before execute via Result
                // so callers get a spanned diagnostic immediately.
                return Err(Error::diagnostic(Diagnostic {
                    code: "IRIS-PLAN-REJECTED".into(),
                    message,
                    span,
                    stage: crate::diagnostic::StageKind::Plan,
                    backend: Some(self.capabilities.backend_id.clone()),
                    hint: Some("enable the missing capability or rewrite the VOS operation".into()),
                }));
            }
            nodes.push(PlannedNode {
                op,
                realization,
                note,
            });
        }

        let semantic_hash = hash_ops(&nodes.iter().map(|n| n.op.clone()).collect::<Vec<_>>());
        let envelope = IrEnvelope {
            vos_contract_version: "vos-dev".into(),
            ir_version: IrVersion::PHASE1,
            schema_fingerprint: SchemaFingerprint::unbound(),
            operation_id: format!("op-{:x}", semantic_hash.0),
            effect: EffectKind::Read,
            required_capabilities: required,
            span_start: span.start,
            span_end: span.end,
            semantic_hash,
        };
        envelope.check_version(self.capabilities.ir_version_max)?;
        Ok(PhysicalPlan { envelope, nodes })
    }

    fn classify(&self, op: &PhysicalOp) -> (RealizationClass, Option<String>) {
        let q = &self.capabilities.query;
        match op {
            PhysicalOp::Scan { .. } | PhysicalOp::Collect => (RealizationClass::Native, None),
            PhysicalOp::Filter { .. } => {
                if q.filter_bool || q.filter_cmp {
                    (RealizationClass::Native, None)
                } else {
                    (
                        RealizationClass::Rejected,
                        Some("backend cannot filter rows".into()),
                    )
                }
            }
            PhysicalOp::Project { .. } => {
                if q.project {
                    (RealizationClass::Native, None)
                } else {
                    (
                        RealizationClass::Rejected,
                        Some("backend cannot project fields".into()),
                    )
                }
            }
            PhysicalOp::Sort { .. } => {
                if q.sort {
                    (RealizationClass::Native, None)
                } else {
                    (
                        RealizationClass::Rejected,
                        Some("backend cannot sort rows".into()),
                    )
                }
            }
            PhysicalOp::Skip { .. } | PhysicalOp::Take { .. } => {
                if q.page {
                    (RealizationClass::Native, None)
                } else {
                    (
                        RealizationClass::Rejected,
                        Some("backend cannot page with skip/take".into()),
                    )
                }
            }
        }
    }
}
