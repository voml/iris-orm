//! In-memory reference store and dual execution paths.

use std::collections::BTreeMap;

use iris_ir::{CmpOp, LiteralKind, PhysicalOp, PhysicalPlan, Pred};

use crate::error::{Error, Result};
use crate::lower;
use crate::value::{Row, Value};

/// In-memory multi-table store used as the Phase 1 reference adapter.
#[derive(Debug, Clone, Default)]
pub struct ReferenceStore {
    tables: BTreeMap<String, Vec<Row>>,
}

impl ReferenceStore {
    /// Empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace all rows for a table.
    pub fn seed(&mut self, table: impl Into<String>, rows: Vec<Row>) {
        self.tables.insert(table.into(), rows);
    }

    /// Execute a physical plan against this store.
    pub fn execute_plan(&self, plan: &PhysicalPlan) -> Result<Vec<Row>> {
        self.execute_plan_with_budget(plan, None)
    }

    /// Execute a physical plan, optionally enforcing a compensation row budget.
    pub fn execute_plan_with_budget(
        &self,
        plan: &PhysicalPlan,
        budget: Option<&crate::CompensationBudget>,
    ) -> Result<Vec<Row>> {
        if plan.is_rejected() {
            return Err(Error::Runtime(
                plan.rejection_note().unwrap_or("plan rejected").to_string(),
            ));
        }
        let mut rows: Vec<Row> = Vec::new();
        for node in &plan.nodes {
            match &node.op {
                PhysicalOp::Scan { table } => {
                    rows = self
                        .tables
                        .get(table)
                        .cloned()
                        .ok_or_else(|| Error::Runtime(format!("unknown table `{table}`")))?;
                }
                PhysicalOp::Filter { predicate } => {
                    rows.retain(|row| eval_pred(predicate, row));
                }
                PhysicalOp::Project { fields } => {
                    rows = rows
                        .into_iter()
                        .map(|row| {
                            let mut out = Row::new();
                            for f in fields {
                                let src = f.from.as_deref().unwrap_or(f.name.as_str());
                                if let Some(v) = row.get(src) {
                                    out.insert(f.name.clone(), v.clone());
                                }
                            }
                            out
                        })
                        .collect();
                }
                PhysicalOp::Sort { keys } => {
                    rows.sort_by(|a, b| {
                        for key in keys {
                            let av = a.get(&key.field).unwrap_or(&Value::Null);
                            let bv = b.get(&key.field).unwrap_or(&Value::Null);
                            let ord = av.cmp(bv);
                            let ord = if key.ascending { ord } else { ord.reverse() };
                            if ord != std::cmp::Ordering::Equal {
                                return ord;
                            }
                        }
                        std::cmp::Ordering::Equal
                    });
                }
                PhysicalOp::Skip { count } => {
                    let n = (*count as usize).min(rows.len());
                    rows = rows.into_iter().skip(n).collect();
                }
                PhysicalOp::Take { count } => {
                    rows.truncate(*count as usize);
                }
                PhysicalOp::Collect => {}
            }
        }
        if let Some(budget) = budget {
            budget
                .enforce_rows(rows.len() as u64)
                .map_err(Error::Runtime)?;
        }
        Ok(rows)
    }

    /// Interpret a VOS program directly for conformance.
    pub fn interpret_source(&self, source: &str) -> Result<Vec<Row>> {
        let program = vos::parse_program(source).map_err(|d| {
            Error::Runtime(format!(
                "parse failed: {}",
                d.errors
                    .first()
                    .map(|e| e.message.as_str())
                    .unwrap_or("unknown")
            ))
        })?;
        self.interpret_program(&program)
    }

    /// Interpret a parsed program by lowering then executing (AST->ops path).
    ///
    /// Capability rejection is tested on the planner path; this path proves
    /// physical-op semantics independently of capability gating.
    pub fn interpret_program(&self, program: &vos::ast::expr::Program) -> Result<Vec<Row>> {
        let (ops, _span) = lower::lower_program(program)?;
        let plan = PhysicalPlan {
            envelope: iris_ir::IrEnvelope {
                vos_contract_version: "vos-dev".into(),
                ir_version: iris_ir::IrVersion::PHASE1,
                schema_fingerprint: iris_ir::SchemaFingerprint::unbound(),
                operation_id: "interpret".into(),
                effect: iris_ir::EffectKind::Read,
                required_capabilities: Vec::new(),
                span_start: 0,
                span_end: 0,
                semantic_hash: iris_ir::SemanticHash(0),
            },
            nodes: ops
                .into_iter()
                .map(|op| iris_ir::PlannedNode {
                    op,
                    realization: iris_ir::RealizationClass::Native,
                    note: None,
                })
                .collect(),
        };
        self.execute_plan(&plan)
    }
}

fn eval_pred(pred: &Pred, row: &Row) -> bool {
    match pred {
        Pred::FieldBool { field, value } => match row.get(field) {
            Some(Value::Bool(b)) => b == value,
            _ => false,
        },
        Pred::FieldCmp {
            field,
            op,
            literal,
            kind,
        } => {
            let left = row.get(field).cloned().unwrap_or(Value::Null);
            let right = decode_literal(literal, *kind);
            cmp_values(&left, *op, &right)
        }
        Pred::And(a, b) => eval_pred(a, row) && eval_pred(b, row),
        Pred::Or(a, b) => eval_pred(a, row) || eval_pred(b, row),
    }
}

fn decode_literal(text: &str, kind: LiteralKind) -> Value {
    match kind {
        LiteralKind::Null => Value::Null,
        LiteralKind::Bool => Value::Bool(text == "true"),
        LiteralKind::Int => Value::Int(text.parse().unwrap_or(0)),
        LiteralKind::Str => Value::Str(text.to_string()),
    }
}

fn cmp_values(left: &Value, op: CmpOp, right: &Value) -> bool {
    let ord = left.cmp(right);
    match op {
        CmpOp::Eq => ord == std::cmp::Ordering::Equal,
        CmpOp::Ne => ord != std::cmp::Ordering::Equal,
        CmpOp::Lt => ord == std::cmp::Ordering::Less,
        CmpOp::Le => ord != std::cmp::Ordering::Greater,
        CmpOp::Gt => ord == std::cmp::Ordering::Greater,
        CmpOp::Ge => ord != std::cmp::Ordering::Less,
    }
}

/// Build a row from field pairs.
pub fn row_from_pairs(pairs: &[(&str, Value)]) -> Row {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), v.clone()))
        .collect()
}
