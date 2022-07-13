//! Lower VOS expression pipelines into Iris physical ops.

use iris_ir::{CmpOp, LiteralKind, PhysicalOp, Pred, ProjectField, SortKey, WriteField};
use vos::ast::{
    Literal, Program, Span,
    expr::{BinaryOp, Expr, PathSep, ProjItem, Stmt},
};

use crate::{
    diagnostic::Diagnostic,
    error::{Error, Result},
};

enum ExecBoundary {
    Collect(Expr),
    Delete(Expr),
    Patch(Expr, Expr),
    Insert(Expr),
}

/// Lower a VOS program into an ordered physical op list + root span.
pub fn lower_program(program: &Program) -> Result<(Vec<PhysicalOp>, Span)> {
    let mut bindings: Vec<(String, Expr)> = Vec::new();
    for stmt in &program.statements {
        match stmt {
            Stmt::Let(let_) => bindings.push((let_.name.clone(), let_.value.clone())),
            Stmt::Expr(expr) => {
                return lower_exec_expr(expr, &bindings);
            }
            _ => {
                return Err(Error::diagnostic(Diagnostic::plan_rejected("unsupported statement in Phase 1 program", program.span, None)));
            }
        }
    }
    let Some(result) = &program.result
    else {
        return Err(Error::diagnostic(Diagnostic::plan_rejected(
            "program has no result expression to execute",
            program.span,
            Some("end with a `.collect()` pipeline or a write operation".into()),
        )));
    };
    lower_exec_expr(result, &bindings)
}

fn lower_exec_expr(expr: &Expr, bindings: &[(String, Expr)]) -> Result<(Vec<PhysicalOp>, Span)> {
    let expr = expand_bindings(expr, bindings)?;
    let (expr, projection) = peel_map(&expr)?;
    let boundary_span = expr_span(&expr);

    match detect_boundary(&expr)? {
        ExecBoundary::Collect(pipeline) => {
            let mut ops = lower_read_pipeline(&pipeline)?;
            if let Some(proj) = projection {
                ops.push(PhysicalOp::Project { fields: lower_projection(&proj)? });
            }
            ops.push(PhysicalOp::Collect);
            Ok((ops, boundary_span))
        }
        ExecBoundary::Delete(pipeline) => {
            if projection.is_some() {
                return Err(Error::diagnostic(Diagnostic::plan_rejected(
                    "`.delete()` cannot be followed by `.map()`",
                    boundary_span,
                    None,
                )));
            }
            let (table, filter) = lower_filter_pipeline(&pipeline)?;
            Ok((vec![PhysicalOp::Delete { table, filter }], boundary_span))
        }
        ExecBoundary::Patch(pipeline, patch_obj) => {
            let (table, filter) = lower_filter_pipeline(&pipeline)?;
            let fields = lower_write_fields(&patch_obj, boundary_span)?;
            let mut ops = vec![PhysicalOp::Patch { table, filter, fields }];
            if let Some(proj) = projection {
                ops.push(PhysicalOp::Project { fields: lower_projection(&proj)? });
            }
            ops.push(PhysicalOp::Collect);
            Ok((ops, boundary_span))
        }
        ExecBoundary::Insert(insert_expr) => {
            let (table, fields) = lower_insert_call(&insert_expr)?;
            let mut ops = vec![PhysicalOp::Insert { table, fields }];
            if let Some(proj) = projection {
                ops.push(PhysicalOp::Project { fields: lower_projection(&proj)? });
            }
            ops.push(PhysicalOp::Collect);
            Ok((ops, boundary_span))
        }
    }
}

fn peel_map(expr: &Expr) -> Result<(Expr, Option<Expr>)> {
    match expr {
        Expr::Call { callee, args, span } => match callee.as_ref() {
            Expr::Member { object, name, .. } if name == "map" => {
                let proj = args
                    .first()
                    .ok_or_else(|| Error::diagnostic(Diagnostic::plan_rejected("`.map` requires a projection lambda", *span, None)))?;
                Ok((object.as_ref().clone(), Some(proj.clone())))
            }
            _ => Ok((expr.clone(), None)),
        },
        _ => Ok((expr.clone(), None)),
    }
}

fn detect_boundary(expr: &Expr) -> Result<ExecBoundary> {
    match expr {
        Expr::Call { callee, args, span } => match callee.as_ref() {
            Expr::Member { object, name, .. } if name == "collect" => {
                if !args.is_empty() {
                    return Err(Error::diagnostic(Diagnostic::plan_rejected("`.collect()` takes no arguments", *span, None)));
                }
                Ok(ExecBoundary::Collect(object.as_ref().clone()))
            }
            Expr::Member { object, name, .. } if name == "delete" => {
                if !args.is_empty() {
                    return Err(Error::diagnostic(Diagnostic::plan_rejected("`.delete()` takes no arguments", *span, None)));
                }
                Ok(ExecBoundary::Delete(object.as_ref().clone()))
            }
            Expr::Member { object, name, .. } if name == "patch" => {
                let patch_obj = args
                    .first()
                    .ok_or_else(|| Error::diagnostic(Diagnostic::plan_rejected("`.patch` requires a field object", *span, None)))?;
                Ok(ExecBoundary::Patch(object.as_ref().clone(), patch_obj.clone()))
            }
            Expr::Member { name, sep, .. } if name == "insert" && *sep == PathSep::ColonColon => {
                Ok(ExecBoundary::Insert(expr.clone()))
            }
            _ => Err(Error::diagnostic(Diagnostic::plan_rejected(
                "unsupported execution boundary",
                *span,
                Some("end reads with `.collect()`, writes with `::insert`, `.patch`, or `.delete()`".into()),
            ))),
        },
        _ => Err(Error::diagnostic(Diagnostic::plan_rejected(
            "unsupported execution boundary",
            expr_span(expr),
            Some("end reads with `.collect()`, writes with `::insert`, `.patch`, or `.delete()`".into()),
        ))),
    }
}

fn lower_insert_call(expr: &Expr) -> Result<(String, Vec<WriteField>)> {
    let Expr::Call { callee, args, span } = expr
    else {
        return Err(Error::diagnostic(Diagnostic::plan_rejected("insert must be a static `Entity::insert({ … })` call", expr_span(expr), None)));
    };
    let Expr::Member { object, name, sep, .. } = callee.as_ref()
    else {
        return Err(Error::diagnostic(Diagnostic::plan_rejected("insert must be a static `Entity::insert({ … })` call", *span, None)));
    };
    if name != "insert" || *sep != PathSep::ColonColon {
        return Err(Error::diagnostic(Diagnostic::plan_rejected("insert must use `Entity::insert({ … })`", *span, None)));
    }
    let table = match object.as_ref() {
        Expr::Name { name, .. } => name.clone(),
        other => {
            return Err(Error::diagnostic(Diagnostic::plan_rejected(
                "insert table must be a bare entity name",
                expr_span(other),
                None,
            )));
        }
    };
    let row = args
        .first()
        .ok_or_else(|| Error::diagnostic(Diagnostic::plan_rejected("`::insert` requires one row object argument", *span, None)))?;
    let fields = lower_write_fields(row, *span)?;
    Ok((table, fields))
}

fn lower_filter_pipeline(expr: &Expr) -> Result<(String, Option<Pred>)> {
    match expr {
        Expr::Call { callee, args, span } => match callee.as_ref() {
            Expr::Member { object, name, .. } if name == "filter" || name == "where" => {
                let (table, _) = lower_filter_pipeline(object)?;
                let pred_expr = args
                    .first()
                    .ok_or_else(|| Error::diagnostic(Diagnostic::plan_rejected(format!("`.{name}` requires a predicate"), *span, None)))?;
                let predicate = lower_predicate(pred_expr)?;
                Ok((table, Some(predicate)))
            }
            _ => Err(Error::diagnostic(Diagnostic::plan_rejected("write pipeline must start from a table filter", *span, None))),
        },
        Expr::Name { name, .. } => Ok((name.clone(), None)),
        other => Err(Error::diagnostic(Diagnostic::plan_rejected(
            "write pipeline must start from a table name",
            expr_span(other),
            None,
        ))),
    }
}

fn lower_write_fields(expr: &Expr, span: Span) -> Result<Vec<WriteField>> {
    let fields = match expr {
        Expr::AnonObject { fields, .. } | Expr::TypedObject { fields, .. } => fields,
        other => {
            return Err(Error::diagnostic(Diagnostic::plan_rejected(
                "write field assignments must be an object literal",
                expr_span(other),
                None,
            )));
        }
    };
    let mut out = Vec::with_capacity(fields.len());
    for init in fields {
        if init.is_shorthand() {
            return Err(Error::diagnostic(Diagnostic::plan_rejected(
                "write field assignments require explicit values",
                init.span,
                None,
            )));
        }
        let value = init.value.as_ref().expect("checked shorthand");
        let mut field = lower_write_field(value, init.span)?;
        field.name = init.name.clone();
        out.push(field);
    }
    if out.is_empty() {
        return Err(Error::diagnostic(Diagnostic::plan_rejected("write requires at least one field assignment", span, None)));
    }
    Ok(out)
}

fn lower_write_field(expr: &Expr, span: Span) -> Result<WriteField> {
    match expr {
        Expr::Literal(Literal::Bool(b)) => Ok(WriteField {
            name: String::new(),
            literal: b.to_string(),
            kind: LiteralKind::Bool,
        }),
        Expr::Literal(Literal::Int(t)) => Ok(WriteField {
            name: String::new(),
            literal: t.clone(),
            kind: LiteralKind::Int,
        }),
        Expr::Literal(Literal::String(s)) => Ok(WriteField {
            name: String::new(),
            literal: s.clone(),
            kind: LiteralKind::Str,
        }),
        Expr::Literal(Literal::Null) => Ok(WriteField {
            name: String::new(),
            literal: "null".into(),
            kind: LiteralKind::Null,
        }),
        Expr::Literal(Literal::Float(t)) => Ok(WriteField {
            name: String::new(),
            literal: t.clone(),
            kind: LiteralKind::Str,
        }),
        Expr::Literal(Literal::Ident(t)) => Ok(WriteField {
            name: String::new(),
            literal: t.clone(),
            kind: LiteralKind::Str,
        }),
        _ => Err(Error::diagnostic(Diagnostic::plan_rejected(
            "write field values must be literals after parameter binding",
            span,
            None,
        ))),
    }
}

fn expand_bindings(expr: &Expr, bindings: &[(String, Expr)]) -> Result<Expr> {
    match expr {
        Expr::Name { name, .. } => {
            if let Some((_, value)) = bindings.iter().rev().find(|(n, _)| n == name) {
                expand_bindings(value, bindings)
            }
            else {
                Ok(expr.clone())
            }
        }
        Expr::Member { object, name, sep, span } => {
            Ok(Expr::Member { object: Box::new(expand_bindings(object, bindings)?), name: name.clone(), sep: *sep, span: *span })
        }
        Expr::Call { callee, args, span } => Ok(Expr::Call {
            callee: Box::new(expand_bindings(callee, bindings)?),
            args: args.iter().map(|a| expand_bindings(a, bindings)).collect::<Result<Vec<_>>>()?,
            span: *span,
        }),
        other => Ok(other.clone()),
    }
}

fn lower_read_pipeline(expr: &Expr) -> Result<Vec<PhysicalOp>> {
    let mut methods: Vec<(&str, &[Expr], Span)> = Vec::new();
    let mut cur = expr;
    loop {
        match cur {
            Expr::Call { callee, args, span } => match callee.as_ref() {
                Expr::Member { object, name, .. } => {
                    methods.push((name.as_str(), args.as_slice(), *span));
                    cur = object.as_ref();
                }
                _ => {
                    return Err(Error::diagnostic(Diagnostic::plan_rejected("unsupported call shape in query pipeline", *span, None)));
                }
            },
            Expr::Member { object, name, span, .. } if name == "all" => {
                methods.push(("all", &[], *span));
                cur = object.as_ref();
            }
            Expr::Name { name, span } => {
                let mut ops = vec![PhysicalOp::Scan { table: name.clone() }];
                for (method, args, mspan) in methods.into_iter().rev() {
                    push_read_method(&mut ops, method, args, mspan)?;
                }
                let _ = span;
                return Ok(ops);
            }
            other => {
                return Err(Error::diagnostic(Diagnostic::plan_rejected(
                    "query pipeline must start from a table name",
                    expr_span(other),
                    None,
                )));
            }
        }
    }
}

fn push_read_method(ops: &mut Vec<PhysicalOp>, method: &str, args: &[Expr], span: Span) -> Result<()> {
    match method {
        "all" => Ok(()),
        "filter" | "where" => {
            let pred_expr = args
                .first()
                .ok_or_else(|| Error::diagnostic(Diagnostic::plan_rejected(format!("`.{method}` requires a predicate"), span, None)))?;
            let predicate = lower_predicate(pred_expr)?;
            ops.push(PhysicalOp::Filter { predicate });
            Ok(())
        }
        "map" => {
            let proj = args.first().ok_or_else(|| Error::diagnostic(Diagnostic::plan_rejected("`.map` requires a projection", span, None)))?;
            let fields = lower_projection(proj)?;
            ops.push(PhysicalOp::Project { fields });
            Ok(())
        }
        "sort_by" => {
            let key =
                args.first().ok_or_else(|| Error::diagnostic(Diagnostic::plan_rejected("`.sort_by` requires a key lambda", span, None)))?;
            let field = lower_field_lambda(key)?;
            ops.push(PhysicalOp::Sort { keys: vec![SortKey { field, ascending: true }] });
            Ok(())
        }
        "sort_by_desc" => {
            let key = args
                .first()
                .ok_or_else(|| Error::diagnostic(Diagnostic::plan_rejected("`.sort_by_desc` requires a key lambda", span, None)))?;
            let field = lower_field_lambda(key)?;
            ops.push(PhysicalOp::Sort { keys: vec![SortKey { field, ascending: false }] });
            Ok(())
        }
        "skip" => {
            let count = literal_u64(args.first(), span, "skip")?;
            ops.push(PhysicalOp::Skip { count });
            Ok(())
        }
        "take" => {
            let count = literal_u64(args.first(), span, "take")?;
            ops.push(PhysicalOp::Take { count });
            Ok(())
        }
        "insert" | "update" | "patch" | "delete" => Err(Error::diagnostic(Diagnostic::plan_rejected(
            format!("write method `.{method}` is not supported in a read pipeline"),
            span,
            Some("use `::insert`, `.patch`, or `.delete()` as the execution boundary".into()),
        ))),
        other => Err(Error::diagnostic(Diagnostic::plan_rejected(
            format!("unsupported pipeline method `.{other}`"),
            span,
            Some("use `.filter(x => …)` or `.where(…)` for predicates".into()),
        ))),
    }
}

fn lower_predicate(expr: &Expr) -> Result<Pred> {
    let body = match expr {
        Expr::Lambda(lambda) => lambda.body.as_ref(),
        other => other,
    };
    lower_pred_body(body)
}

fn lower_pred_body(expr: &Expr) -> Result<Pred> {
    match expr {
        Expr::Binary { op: BinaryOp::And, left, right, .. } => {
            Ok(Pred::And(Box::new(lower_pred_body(left)?), Box::new(lower_pred_body(right)?)))
        }
        Expr::Binary { op: BinaryOp::Or, left, right, .. } => Ok(Pred::Or(Box::new(lower_pred_body(left)?), Box::new(lower_pred_body(right)?))),
        Expr::Binary { op, left, right, span } => {
            let field = match left.as_ref() {
                Expr::Member { name, .. } | Expr::Name { name, .. } => name.clone(),
                _ => {
                    return Err(Error::diagnostic(Diagnostic::plan_rejected(
                        "Phase 1 filters must compare a field access on the left",
                        *span,
                        None,
                    )));
                }
            };
            let (literal, kind) = match right.as_ref() {
                Expr::Literal(Literal::Bool(b)) => (b.to_string(), LiteralKind::Bool),
                Expr::Literal(Literal::Int(t)) => (t.clone(), LiteralKind::Int),
                Expr::Literal(Literal::String(s)) => (s.clone(), LiteralKind::Str),
                Expr::Literal(Literal::Null) => ("null".into(), LiteralKind::Null),
                Expr::Literal(Literal::Float(t)) => (t.clone(), LiteralKind::Str),
                Expr::Literal(Literal::Ident(t)) => (t.clone(), LiteralKind::Str),
                _ => {
                    return Err(Error::diagnostic(Diagnostic::plan_rejected("Phase 1 filters require a literal right-hand side", *span, None)));
                }
            };
            if matches!(op, BinaryOp::Eq) && kind == LiteralKind::Bool {
                let value = literal == "true";
                return Ok(Pred::FieldBool { field, value });
            }
            let cmp = match op {
                BinaryOp::Eq => CmpOp::Eq,
                BinaryOp::Ne => CmpOp::Ne,
                BinaryOp::Lt => CmpOp::Lt,
                BinaryOp::Le => CmpOp::Le,
                BinaryOp::Gt => CmpOp::Gt,
                BinaryOp::Ge => CmpOp::Ge,
                _ => {
                    return Err(Error::diagnostic(Diagnostic::plan_rejected("unsupported comparison in filter", *span, None)));
                }
            };
            Ok(Pred::FieldCmp { field, op: cmp, literal, kind })
        }
        Expr::Member { name, span, .. } | Expr::Name { name, span } => {
            let _ = span;
            Ok(Pred::FieldBool { field: name.clone(), value: true })
        }
        other => Err(Error::diagnostic(Diagnostic::plan_rejected("unsupported predicate shape", expr_span(other), None))),
    }
}

fn lower_field_lambda(expr: &Expr) -> Result<String> {
    let body = match expr {
        Expr::Lambda(lambda) => lambda.body.as_ref(),
        other => other,
    };
    match body {
        Expr::Member { name, .. } => Ok(name.clone()),
        other => Err(Error::diagnostic(Diagnostic::plan_rejected("Phase 1 sort key must be a field access lambda", expr_span(other), None))),
    }
}

fn lower_projection(expr: &Expr) -> Result<Vec<ProjectField>> {
    let body = match expr {
        Expr::Lambda(lambda) => lambda.body.as_ref(),
        other => other,
    };
    match body {
        Expr::StructProj { items, span, .. } => {
            let mut fields = Vec::new();
            for item in items {
                match item {
                    ProjItem::Star { .. } => {
                        return Err(Error::diagnostic(Diagnostic::plan_rejected(
                            "Phase 1 projection does not expand `*` yet",
                            *span,
                            Some("enumerate fields explicitly".into()),
                        )));
                    }
                    ProjItem::Field(init) => {
                        let from = match &init.value {
                            None => None,
                            Some(Expr::Name { name, .. }) | Some(Expr::Member { name, .. }) => {
                                if name == &init.name {
                                    None
                                }
                                else {
                                    Some(name.clone())
                                }
                            }
                            Some(other) => {
                                return Err(Error::diagnostic(Diagnostic::plan_rejected(
                                    "Phase 1 projection values must be field refs",
                                    expr_span(other),
                                    None,
                                )));
                            }
                        };
                        fields.push(ProjectField { name: init.name.clone(), from });
                    }
                    _ => {
                        return Err(Error::diagnostic(Diagnostic::plan_rejected("unsupported projection item", *span, None)));
                    }
                }
            }
            Ok(fields)
        }
        other => Err(Error::diagnostic(Diagnostic::plan_rejected("Phase 1 `.map` expects `x => x.{ ... }`", expr_span(other), None))),
    }
}

fn literal_u64(expr: Option<&Expr>, span: Span, method: &str) -> Result<u64> {
    let Some(expr) = expr
    else {
        return Err(Error::diagnostic(Diagnostic::plan_rejected(format!("`.{method}` requires a count argument"), span, None)));
    };
    match expr {
        Expr::Literal(Literal::Int(t)) => {
            t.parse::<u64>().map_err(|_| Error::diagnostic(Diagnostic::plan_rejected(format!("invalid `{method}` count"), span, None)))
        }
        _ => Err(Error::diagnostic(Diagnostic::plan_rejected(format!("`.{method}` count must be an integer literal"), span, None))),
    }
}

fn expr_span(expr: &Expr) -> Span {
    match expr {
        Expr::Literal(_) => Span::empty(0),
        Expr::Name { span, .. }
        | Expr::TypedObject { span, .. }
        | Expr::AnonObject { span, .. }
        | Expr::List { span, .. }
        | Expr::Member { span, .. }
        | Expr::Call { span, .. }
        | Expr::Unary { span, .. }
        | Expr::Binary { span, .. }
        | Expr::StarProj { span, .. }
        | Expr::StructProj { span, .. }
        | Expr::Try { span, .. } => *span,
        Expr::Lambda(l) => l.span,
        _ => Span::empty(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iris_ir::PhysicalOp;

    #[test]
    fn lowers_delete_with_filter() {
        let program = vos::parse_program(r#"User.filter(x => x.user_id == "u1").delete()"#).expect("parse");
        let (ops, _) = lower_program(&program).expect("lower");
        assert!(matches!(
            ops.as_slice(),
            [PhysicalOp::Delete { table, filter: Some(_) }] if table == "User"
        ));
    }

    #[test]
    fn lowers_insert_with_collect() {
        let program = vos::parse_program(
            r#"User::insert({
                user_id: "u1",
                user_name: "ada",
                active: true,
            })"#,
        )
        .expect("parse");
        let (ops, _) = lower_program(&program).expect("lower");
        assert!(matches!(ops.first(), Some(PhysicalOp::Insert { table, .. }) if table == "User"));
        assert!(matches!(ops.last(), Some(PhysicalOp::Collect)));
    }

    #[test]
    fn lowers_patch_with_projection() {
        let program = vos::parse_program(
            r#"User.filter(x => x.user_id == "u1").patch({ user_name: "bob" }).map(x => x.{ user_id, user_name })"#,
        )
        .expect("parse");
        let (ops, _) = lower_program(&program).expect("lower");
        assert!(matches!(ops.first(), Some(PhysicalOp::Patch { table, .. }) if table == "User"));
        assert!(ops.iter().any(|op| matches!(op, PhysicalOp::Project { .. })));
        assert!(matches!(ops.last(), Some(PhysicalOp::Collect)));
    }
}
