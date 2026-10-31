//! PhysicalPlan / RowWrite → YYDB VOS text + host-bound typed parameters.
//!
//! YYDB's VOS filter surface accepts only `Bool` / `Int` / `String` / `Null`
//! literals and `$param` references — there is no float or payload literal
//! form — so every `FieldCmp` right-hand side is bound as a typed parameter
//! and decoded strictly from its canonical wire encoding. VOS writes are
//! insert-only, so `Patch` / `Delete` fail closed.

use std::collections::BTreeMap;

use iris_ir::{CmpOp, LiteralKind, PhysicalOp, PhysicalPlan, Pred};
use iris_types::{RowWrite, Value};

use crate::{Error, Result};

/// A rendered VOS program with its host-bound typed parameters.
pub(crate) struct Rendered {
    /// VOS source text executed on the YYDB connection.
    pub source: String,
    /// Typed bindings keyed by the `$name` slots appearing in [`Self::source`].
    pub params: BTreeMap<String, Value>,
    /// True when the program is a write (no result rows come back).
    pub is_write: bool,
}

/// Render a planned read pipeline or single insert into VOS text.
pub(crate) fn render_plan(plan: &PhysicalPlan) -> Result<Rendered> {
    if plan.nodes.is_empty() {
        return Err(Error::Policy("empty plan cannot execute on YYDB".into()));
    }
    if let PhysicalOp::Insert { table, fields } = &plan.nodes.first().expect("checked non-empty").op {
        for node in &plan.nodes[1..] {
            if !matches!(node.op, PhysicalOp::Collect) {
                return Err(Error::Policy("YYDB insert plans may only carry a trailing `.collect()` boundary".into()));
            }
        }
        if fields.is_empty() {
            return Err(Error::Policy(format!("insert into `{table}` carries no fields")));
        }
        return render_insert(table, fields.iter().map(|field| (field.name.as_str(), field.literal.as_str(), field.kind)));
    }
    let mut out = String::new();
    let mut params = BTreeMap::new();
    let mut slot = 0u32;
    let mut saw_scan = false;
    let mut saw_collect = false;
    for node in &plan.nodes {
        match &node.op {
            PhysicalOp::Scan { table } => {
                if saw_scan {
                    return Err(Error::Policy(format!("plan scans `{table}` more than once")));
                }
                out.push_str(table);
                saw_scan = true;
            }
            PhysicalOp::Filter { predicate } => {
                require_scan(saw_scan, "filter")?;
                out.push_str(&format!(".filter(x => {})", render_pred(predicate, &mut params, &mut slot)?));
            }
            PhysicalOp::Project { fields } => {
                require_scan(saw_scan, "project")?;
                if fields.is_empty() {
                    return Err(Error::Policy("plan projects no fields".into()));
                }
                out.push_str(".map(x => x.{ ");
                for field in fields {
                    match &field.from {
                        Some(from) if *from != field.name => out.push_str(&format!("{}: {}, ", field.name, from)),
                        _ => out.push_str(&format!("{}, ", field.name)),
                    }
                }
                out.push_str("})");
            }
            PhysicalOp::Sort { keys } => {
                require_scan(saw_scan, "sort")?;
                if keys.is_empty() {
                    return Err(Error::Policy("plan sorts with no keys".into()));
                }
                for key in keys.iter().rev() {
                    if key.ascending {
                        out.push_str(&format!(".sort_by(x => x.{})", key.field));
                    }
                    else {
                        out.push_str(&format!(".sort_by_desc(x => x.{})", key.field));
                    }
                }
            }
            PhysicalOp::Skip { count } => {
                require_scan(saw_scan, "skip")?;
                out.push_str(&format!(".skip({count})"));
            }
            PhysicalOp::Take { count } => {
                require_scan(saw_scan, "take")?;
                out.push_str(&format!(".take({count})"));
            }
            PhysicalOp::Collect => {
                require_scan(saw_scan, "collect")?;
                out.push_str(".collect()");
                saw_collect = true;
            }
            PhysicalOp::Insert { .. } => {
                return Err(Error::Policy("YYDB insert plans must start with the insert".into()));
            }
            PhysicalOp::Patch { .. } => {
                return Err(Error::Policy("YYDB VOS writes are insert-only; patch has no native form on this connector".into()));
            }
            PhysicalOp::Delete { .. } => {
                return Err(Error::Policy("YYDB VOS writes are insert-only; delete has no native form on this connector".into()));
            }
        }
    }
    if !saw_collect {
        return Err(Error::Policy("YYDB read plans must end with `.collect()`".into()));
    }
    Ok(Rendered { source: out, params, is_write: false })
}

/// Render a full row write into the static `Table::insert({ field: $field, … })` form.
pub(crate) fn render_row_write(write: &RowWrite) -> Result<Rendered> {
    if write.fields.is_empty() {
        return Err(Error::Policy(format!("insert into `{}` carries no fields", write.table)));
    }
    let mut params = BTreeMap::new();
    let mut body = String::new();
    for (name, value) in &write.fields {
        body.push_str(&format!("{name}: ${name}, "));
        params.insert(name.clone(), value.clone());
    }
    Ok(Rendered { source: format!("{}::insert({{{body}}})", write.table), params, is_write: true })
}

fn render_insert<'a>(table: &str, fields: impl Iterator<Item = (&'a str, &'a str, LiteralKind)>) -> Result<Rendered> {
    let mut params = BTreeMap::new();
    let mut body = String::new();
    for (name, literal, kind) in fields {
        let value = decode_literal(literal, kind, &format!("insert field `{name}`"))?;
        body.push_str(&format!("{name}: ${name}, "));
        params.insert(name.to_owned(), value);
    }
    Ok(Rendered { source: format!("{table}::insert({{{body}}})"), params, is_write: true })
}

fn require_scan(saw_scan: bool, stage: &str) -> Result<()> {
    if saw_scan {
        Ok(())
    }
    else {
        Err(Error::Policy(format!("plan applies `{stage}` before scanning a table")))
    }
}

fn render_pred(pred: &Pred, params: &mut BTreeMap<String, Value>, slot: &mut u32) -> Result<String> {
    match pred {
        Pred::FieldBool { field, value } => {
            if *value {
                Ok(format!("x.{field}"))
            }
            else {
                Ok(format!("x.{field} == false"))
            }
        }
        Pred::FieldCmp { field, op, literal, kind } => {
            let value = decode_literal(literal, *kind, &format!("filter field `{field}`"))?;
            let name = format!("p{slot}");
            *slot += 1;
            params.insert(name.clone(), value);
            Ok(format!("x.{field} {} ${name}", cmp_text(*op)))
        }
        Pred::And(left, right) => Ok(format!("({} && {})", render_pred(left, params, slot)?, render_pred(right, params, slot)?)),
        Pred::Or(left, right) => Ok(format!("({} || {})", render_pred(left, params, slot)?, render_pred(right, params, slot)?)),
    }
}

fn cmp_text(op: CmpOp) -> &'static str {
    match op {
        CmpOp::Eq => "==",
        CmpOp::Ne => "!=",
        CmpOp::Lt => "<",
        CmpOp::Le => "<=",
        CmpOp::Gt => ">",
        CmpOp::Ge => ">=",
    }
}

/// Strictly decode a plan literal from its canonical wire encoding into the
/// typed Iris value a host binds as a `$param`.
pub(crate) fn decode_literal(literal: &str, kind: LiteralKind, context: &str) -> Result<Value> {
    let decoded = match kind {
        LiteralKind::Bool => match literal {
            "true" => Some(Value::Bool(true)),
            "false" => Some(Value::Bool(false)),
            _ => None,
        },
        LiteralKind::Int => literal.parse::<i64>().ok().map(Value::Int),
        LiteralKind::Str => Some(Value::Str(literal.to_owned())),
        LiteralKind::Decimal => Some(Value::Decimal(literal.to_owned())),
        LiteralKind::Date => Some(Value::Date(literal.to_owned())),
        LiteralKind::Time => Some(Value::Time(literal.to_owned())),
        LiteralKind::DateTime => Some(Value::DateTime(literal.to_owned())),
        LiteralKind::Null => {
            if literal == "null" {
                Some(Value::Null)
            }
            else {
                None
            }
        }
        LiteralKind::Bytes => iris_types::hex_to_bytes(literal).map(Value::Bytes),
        LiteralKind::Vector => serde_json::from_str::<serde_json::Value>(literal)
            .ok()
            .as_ref()
            .and_then(iris_types::vector_from_tagged_json)
            .map(Value::Vector),
        LiteralKind::File => serde_json::from_str::<serde_json::Value>(literal)
            .ok()
            .as_ref()
            .and_then(iris_types::file_from_tagged_json)
            .map(Value::File),
    };
    decoded.ok_or_else(|| Error::Policy(format!("{context} carries a `{kind:?}` literal that is not in its canonical wire form")))
}
