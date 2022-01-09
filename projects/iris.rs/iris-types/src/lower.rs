//! Lower VOS expression pipelines into Iris physical ops.

use iris_ir::{CmpOp, LiteralKind, PhysicalOp, Pred, ProjectField, SortKey};
use vos::ast::expr::{BinaryOp, Expr, ProjItem, Stmt};
use vos::ast::{Literal, Program, Span};

use crate::diagnostic::Diagnostic;
use crate::error::{Error, Result};

/// Lower a VOS program into an ordered physical op list + root span.
///
/// Phase 1 supports a single read pipeline ending in `.collect()`, optionally
/// bound through `let` aliases.
pub fn lower_program(program: &Program) -> Result<(Vec<PhysicalOp>, Span)> {
    let mut bindings: Vec<(String, Expr)> = Vec::new();
    for stmt in &program.statements {
        match stmt {
            Stmt::Let(let_) => bindings.push((let_.name.clone(), let_.value.clone())),
            Stmt::Expr(expr) => {
                return lower_exec_expr(expr, &bindings);
            }
            _ => {
                return Err(Error::diagnostic(Diagnostic::plan_rejected(
                    "unsupported statement in Phase 1 program",
                    program.span,
                    None,
                )));
            }
        }
    }
    let Some(result) = &program.result else {
        return Err(Error::diagnostic(Diagnostic::plan_rejected(
            "program has no result expression to execute",
            program.span,
            Some("end with a `.collect()` pipeline".into()),
        )));
    };
    lower_exec_expr(result, &bindings)
}

fn lower_exec_expr(expr: &Expr, bindings: &[(String, Expr)]) -> Result<(Vec<PhysicalOp>, Span)> {
    let expr = expand_bindings(expr, bindings)?;
    let (pipeline, boundary_span) = split_collect(&expr)?;
    let ops = lower_pipeline(&pipeline)?;
    let mut out = ops;
    out.push(PhysicalOp::Collect);
    Ok((out, boundary_span))
}

fn expand_bindings(expr: &Expr, bindings: &[(String, Expr)]) -> Result<Expr> {
    match expr {
        Expr::Name { name, .. } => {
            if let Some((_, value)) = bindings.iter().rev().find(|(n, _)| n == name) {
                expand_bindings(value, bindings)
            } else {
                Ok(expr.clone())
            }
        }
        Expr::Member {
            object,
            name,
            sep,
            span,
        } => Ok(Expr::Member {
            object: Box::new(expand_bindings(object, bindings)?),
            name: name.clone(),
            sep: *sep,
            span: *span,
        }),
        Expr::Call { callee, args, span } => Ok(Expr::Call {
            callee: Box::new(expand_bindings(callee, bindings)?),
            args: args
                .iter()
                .map(|a| expand_bindings(a, bindings))
                .collect::<Result<Vec<_>>>()?,
            span: *span,
        }),
        other => Ok(other.clone()),
    }
}

fn split_collect(expr: &Expr) -> Result<(Expr, Span)> {
    match expr {
        Expr::Call { callee, args, span } => match callee.as_ref() {
            Expr::Member { object, name, .. } if name == "collect" => {
                if !args.is_empty() {
                    return Err(Error::diagnostic(Diagnostic::plan_rejected(
                        "`.collect()` takes no arguments",
                        *span,
                        None,
                    )));
                }
                Ok((object.as_ref().clone(), *span))
            }
            _ => Err(Error::diagnostic(Diagnostic::plan_rejected(
                "Phase 1 only executes pipelines ending in `.collect()`",
                *span,
                Some("add `.collect()` as the execution boundary".into()),
            ))),
        },
        other => Err(Error::diagnostic(Diagnostic::plan_rejected(
            "Phase 1 only executes pipelines ending in `.collect()`",
            expr_span(other),
            Some("add `.collect()` as the execution boundary".into()),
        ))),
    }
}

fn lower_pipeline(expr: &Expr) -> Result<Vec<PhysicalOp>> {
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
                    return Err(Error::diagnostic(Diagnostic::plan_rejected(
                        "unsupported call shape in query pipeline",
                        *span,
                        None,
                    )));
                }
            },
            Expr::Member {
                object, name, span, ..
            } if name == "all" => {
                methods.push(("all", &[], *span));
                cur = object.as_ref();
            }
            Expr::Name { name, span } => {
                let mut ops = vec![PhysicalOp::Scan {
                    table: name.clone(),
                }];
                for (method, args, mspan) in methods.into_iter().rev() {
                    push_method(&mut ops, method, args, mspan)?;
                }
                // Ensure scan span is recorded via table name only; ok for Phase 1.
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

fn push_method(ops: &mut Vec<PhysicalOp>, method: &str, args: &[Expr], span: Span) -> Result<()> {
    match method {
        "all" => Ok(()),
        "filter" => {
            let pred_expr = args.first().ok_or_else(|| {
                Error::diagnostic(Diagnostic::plan_rejected(
                    "`.filter` requires a predicate",
                    span,
                    None,
                ))
            })?;
            let predicate = lower_predicate(pred_expr)?;
            ops.push(PhysicalOp::Filter { predicate });
            Ok(())
        }
        "map" => {
            let proj = args.first().ok_or_else(|| {
                Error::diagnostic(Diagnostic::plan_rejected(
                    "`.map` requires a projection",
                    span,
                    None,
                ))
            })?;
            let fields = lower_projection(proj)?;
            ops.push(PhysicalOp::Project { fields });
            Ok(())
        }
        "sort_by" => {
            let key = args.first().ok_or_else(|| {
                Error::diagnostic(Diagnostic::plan_rejected(
                    "`.sort_by` requires a key lambda",
                    span,
                    None,
                ))
            })?;
            let field = lower_field_lambda(key)?;
            ops.push(PhysicalOp::Sort {
                keys: vec![SortKey {
                    field,
                    ascending: true,
                }],
            });
            Ok(())
        }
        "sort_by_desc" => {
            let key = args.first().ok_or_else(|| {
                Error::diagnostic(Diagnostic::plan_rejected(
                    "`.sort_by_desc` requires a key lambda",
                    span,
                    None,
                ))
            })?;
            let field = lower_field_lambda(key)?;
            ops.push(PhysicalOp::Sort {
                keys: vec![SortKey {
                    field,
                    ascending: false,
                }],
            });
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
        "insert" | "update" | "delete" => Err(Error::diagnostic(Diagnostic::plan_rejected(
            format!("write method `.{method}` is not supported by this planner path"),
            span,
            Some("Phase 1 reference path is read-only; enable a write-capable backend".into()),
        ))),
        other => Err(Error::diagnostic(Diagnostic::plan_rejected(
            format!("unsupported pipeline method `.{other}`"),
            span,
            None,
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
        Expr::Binary {
            op: BinaryOp::And,
            left,
            right,
            ..
        } => Ok(Pred::And(
            Box::new(lower_pred_body(left)?),
            Box::new(lower_pred_body(right)?),
        )),
        Expr::Binary {
            op: BinaryOp::Or,
            left,
            right,
            ..
        } => Ok(Pred::Or(
            Box::new(lower_pred_body(left)?),
            Box::new(lower_pred_body(right)?),
        )),
        Expr::Binary {
            op,
            left,
            right,
            span,
        } => {
            let field = match left.as_ref() {
                Expr::Member { name, .. } => name.clone(),
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
                    return Err(Error::diagnostic(Diagnostic::plan_rejected(
                        "Phase 1 filters require a literal right-hand side",
                        *span,
                        None,
                    )));
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
                    return Err(Error::diagnostic(Diagnostic::plan_rejected(
                        "unsupported comparison in filter",
                        *span,
                        None,
                    )));
                }
            };
            Ok(Pred::FieldCmp {
                field,
                op: cmp,
                literal,
                kind,
            })
        }
        Expr::Member { name, span, .. } => {
            // Bare `x.active` treated as `x.active == true`.
            let _ = span;
            Ok(Pred::FieldBool {
                field: name.clone(),
                value: true,
            })
        }
        other => Err(Error::diagnostic(Diagnostic::plan_rejected(
            "unsupported predicate shape",
            expr_span(other),
            None,
        ))),
    }
}

fn lower_field_lambda(expr: &Expr) -> Result<String> {
    let body = match expr {
        Expr::Lambda(lambda) => lambda.body.as_ref(),
        other => other,
    };
    match body {
        Expr::Member { name, .. } => Ok(name.clone()),
        other => Err(Error::diagnostic(Diagnostic::plan_rejected(
            "Phase 1 sort key must be a field access lambda",
            expr_span(other),
            None,
        ))),
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
                                } else {
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
                        fields.push(ProjectField {
                            name: init.name.clone(),
                            from,
                        });
                    }
                    _ => {
                        return Err(Error::diagnostic(Diagnostic::plan_rejected(
                            "unsupported projection item",
                            *span,
                            None,
                        )));
                    }
                }
            }
            Ok(fields)
        }
        other => Err(Error::diagnostic(Diagnostic::plan_rejected(
            "Phase 1 `.map` expects `x => x.{ ... }`",
            expr_span(other),
            None,
        ))),
    }
}

fn literal_u64(expr: Option<&Expr>, span: Span, method: &str) -> Result<u64> {
    let Some(expr) = expr else {
        return Err(Error::diagnostic(Diagnostic::plan_rejected(
            format!("`.{method}` requires a count argument"),
            span,
            None,
        )));
    };
    match expr {
        Expr::Literal(Literal::Int(t)) => t.parse::<u64>().map_err(|_| {
            Error::diagnostic(Diagnostic::plan_rejected(
                format!("invalid `{method}` count"),
                span,
                None,
            ))
        }),
        _ => Err(Error::diagnostic(Diagnostic::plan_rejected(
            format!("`.{method}` count must be an integer literal"),
            span,
            None,
        ))),
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
