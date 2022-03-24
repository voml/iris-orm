//! Managed Push for MySQL (private DDL).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use iris_types::{LogicalChange, LogicalMigrationPlan, ObservedCatalog};
use mysql::PooledConn;
use mysql::prelude::*;
use vos::ast::{BuiltinType, Document, Field, Item, TypeExpr};

use crate::{Error, Result};

/// Report after applying a managed push.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushReport {
    /// Plan id applied.
    pub plan_id: String,
    /// Tables created.
    pub created_tables: Vec<String>,
}

/// Build a non-destructive plan: create missing tables + add missing fields.
pub fn plan_push(document: &Document, observed: &ObservedCatalog) -> Result<LogicalMigrationPlan> {
    let target = fingerprint_document(document);
    let mut changes = Vec::new();
    for item in &document.items {
        let Item::Table(table) = item else {
            continue;
        };
        match observed.table(&table.name) {
            None => {
                changes.push(LogicalChange::CreateTable {
                    vos_table: table.name.clone(),
                });
            }
            Some(obs) => {
                for field in &table.fields {
                    if !obs.columns.iter().any(|c| c.name == field.name) {
                        changes.push(LogicalChange::AddField {
                            vos_table: table.name.clone(),
                            vos_field: field.name.clone(),
                        });
                    }
                }
            }
        }
    }
    Ok(LogicalMigrationPlan {
        id: format!("push-{target}"),
        parent_fingerprint: fingerprint_catalog(observed),
        target_fingerprint: target,
        changes,
        destructive: false,
    })
}

/// Apply a reviewed logical plan by emitting private MySQL DDL.
pub fn apply_push(
    conn: &mut PooledConn,
    plan: &LogicalMigrationPlan,
    document: &Document,
) -> Result<PushReport> {
    if plan.destructive {
        return Err(Error::Policy(
            "refusing to apply destructive plan without explicit policy".into(),
        ));
    }
    let mut created = Vec::new();
    conn.query_drop("START TRANSACTION")?;
    let apply = (|| -> Result<()> {
        for change in &plan.changes {
            match change {
                LogicalChange::CreateTable { vos_table } => {
                    let table = document
                        .items
                        .iter()
                        .find_map(|i| match i {
                            Item::Table(t) if t.name == *vos_table => Some(t),
                            _ => None,
                        })
                        .ok_or_else(|| {
                            Error::Policy(format!(
                                "plan references unknown VOS table `{vos_table}`"
                            ))
                        })?;
                    let ddl = create_table_sql(table)?;
                    conn.query_drop(ddl)?;
                    created.push(vos_table.clone());
                }
                LogicalChange::AddField {
                    vos_table,
                    vos_field,
                } => {
                    let table = document
                        .items
                        .iter()
                        .find_map(|i| match i {
                            Item::Table(t) if t.name == *vos_table => Some(t),
                            _ => None,
                        })
                        .ok_or_else(|| {
                            Error::Policy(format!(
                                "plan references unknown VOS table `{vos_table}`"
                            ))
                        })?;
                    let field = table
                        .fields
                        .iter()
                        .find(|f| f.name == *vos_field)
                        .ok_or_else(|| {
                            Error::Policy(format!(
                                "plan references unknown VOS field `{vos_table}.{vos_field}`"
                            ))
                        })?;
                    if field.is_primary() {
                        return Err(Error::Policy(format!(
                            "refusing AddField for primary key `{vos_table}.{vos_field}` — recreate table"
                        )));
                    }
                    let ddl = add_field_sql(vos_table, field)?;
                    conn.query_drop(ddl)?;
                }
            }
        }
        Ok(())
    })();
    match apply {
        Ok(()) => {
            conn.query_drop("COMMIT")?;
            Ok(PushReport {
                plan_id: plan.id.clone(),
                created_tables: created,
            })
        }
        Err(e) => {
            let _ = conn.query_drop("ROLLBACK");
            Err(e)
        }
    }
}

fn create_table_sql(table: &vos::ast::Table) -> Result<String> {
    let mut cols = Vec::new();
    let mut pks = Vec::new();
    for field in &table.fields {
        let (sql_ty, not_null) = map_field_type(field)?;
        let mut piece = format!("`{}` {sql_ty}", field.name);
        if not_null {
            piece.push_str(" NOT NULL");
        }
        if field.is_primary() {
            pks.push(format!("`{}`", field.name));
        }
        cols.push(piece);
    }
    if pks.is_empty() {
        return Err(Error::Policy(format!(
            "table `{}` has no primary key -- cannot push",
            table.name
        )));
    }
    cols.push(format!("PRIMARY KEY ({})", pks.join(", ")));
    Ok(format!(
        "CREATE TABLE IF NOT EXISTS `{}` ({}) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        table.name,
        cols.join(", ")
    ))
}

fn add_field_sql(table: &str, field: &Field) -> Result<String> {
    let (sql_ty, not_null) = map_field_type_for_add(field)?;
    let mut piece = format!("`{}` {sql_ty}", field.name);
    if not_null {
        // Existing rows need a value when adding NOT NULL columns.
        // Prefer VARCHAR over TEXT so DEFAULT is portable across MySQL versions.
        piece.push_str(" NOT NULL");
        piece.push_str(&format!(" DEFAULT {}", default_literal(field)?));
    } else {
        piece.push_str(" NULL");
    }
    Ok(format!("ALTER TABLE `{table}` ADD COLUMN {piece};"))
}

/// Same as [`map_field_type`], except non-PK utf8 uses VARCHAR so NOT NULL DEFAULT works.
fn map_field_type_for_add(field: &Field) -> Result<(String, bool)> {
    let (sql_ty, not_null) = map_field_type(field)?;
    let (inner, _) = strip_optional(&field.ty);
    let sql_ty = match inner {
        TypeExpr::Builtin(BuiltinType::Utf8)
        | TypeExpr::Builtin(BuiltinType::Utf16)
        | TypeExpr::Builtin(BuiltinType::Decimal)
        | TypeExpr::Builtin(BuiltinType::Date)
        | TypeExpr::Builtin(BuiltinType::Time)
        | TypeExpr::Builtin(BuiltinType::DateTimeUtc)
            if !field.is_primary() =>
        {
            "VARCHAR(512)".into()
        }
        _ => sql_ty,
    };
    Ok((sql_ty, not_null))
}

fn default_literal(field: &Field) -> Result<String> {
    let (inner, _) = strip_optional(&field.ty);
    match inner {
        TypeExpr::Builtin(BuiltinType::Bool) => Ok("0".into()),
        TypeExpr::Builtin(BuiltinType::I8)
        | TypeExpr::Builtin(BuiltinType::U8)
        | TypeExpr::Builtin(BuiltinType::I16)
        | TypeExpr::Builtin(BuiltinType::U16)
        | TypeExpr::Builtin(BuiltinType::I32)
        | TypeExpr::Builtin(BuiltinType::U32)
        | TypeExpr::Builtin(BuiltinType::I64)
        | TypeExpr::Builtin(BuiltinType::U64)
        | TypeExpr::Builtin(BuiltinType::F32)
        | TypeExpr::Builtin(BuiltinType::F64) => Ok("0".into()),
        TypeExpr::Builtin(BuiltinType::Utf8)
        | TypeExpr::Builtin(BuiltinType::Utf16)
        | TypeExpr::Builtin(BuiltinType::Decimal)
        | TypeExpr::Builtin(BuiltinType::Date)
        | TypeExpr::Builtin(BuiltinType::Time)
        | TypeExpr::Builtin(BuiltinType::DateTimeUtc) => Ok("''".into()),
        TypeExpr::Builtin(BuiltinType::Uuid) => Ok("0x00000000000000000000000000000000".into()),
        TypeExpr::Builtin(BuiltinType::Bytes) => Ok("''".into()),
        other => Err(Error::Policy(format!(
            "no AddField default for VOS type {other:?}"
        ))),
    }
}

fn map_field_type(field: &Field) -> Result<(String, bool)> {
    let (inner, optional) = strip_optional(&field.ty);
    let pk = field.is_primary();
    let sql = match inner {
        // MySQL BOOLEAN → TINYINT(1) — document as intentional dialect choice.
        TypeExpr::Builtin(BuiltinType::Bool) => "TINYINT(1)".into(),
        TypeExpr::Builtin(BuiltinType::I8)
        | TypeExpr::Builtin(BuiltinType::U8)
        | TypeExpr::Builtin(BuiltinType::I16)
        | TypeExpr::Builtin(BuiltinType::U16) => "SMALLINT".into(),
        TypeExpr::Builtin(BuiltinType::I32) | TypeExpr::Builtin(BuiltinType::U32) => "INT".into(),
        TypeExpr::Builtin(BuiltinType::I64) | TypeExpr::Builtin(BuiltinType::U64) => {
            "BIGINT".into()
        }
        TypeExpr::Builtin(BuiltinType::F32) => "FLOAT".into(),
        TypeExpr::Builtin(BuiltinType::F64) => "DOUBLE".into(),
        TypeExpr::Builtin(BuiltinType::Utf8)
        | TypeExpr::Builtin(BuiltinType::Utf16)
        | TypeExpr::Builtin(BuiltinType::Decimal)
        | TypeExpr::Builtin(BuiltinType::Date)
        | TypeExpr::Builtin(BuiltinType::Time)
        | TypeExpr::Builtin(BuiltinType::DateTimeUtc) => {
            if pk {
                // utf8mb4 indexable PK (InnoDB max index prefix for VARCHAR).
                "VARCHAR(191)".into()
            } else {
                "TEXT".into()
            }
        }
        TypeExpr::Builtin(BuiltinType::Uuid) => "BINARY(16)".into(),
        TypeExpr::Builtin(BuiltinType::Bytes) => "BLOB".into(),
        TypeExpr::Builtin(_) => {
            return Err(Error::Policy(
                "unsupported builtin VOS type for MySQL push".into(),
            ));
        }
        other => {
            return Err(Error::Policy(format!(
                "unsupported VOS type for MySQL push: {other:?}"
            )));
        }
    };
    Ok((sql, !optional))
}

fn strip_optional(ty: &TypeExpr) -> (&TypeExpr, bool) {
    match ty {
        TypeExpr::Optional(inner) => (inner.as_ref(), true),
        other => (other, false),
    }
}

fn fingerprint_document(document: &Document) -> String {
    let mut h = DefaultHasher::new();
    format!("{document:?}").hash(&mut h);
    format!("{:x}", h.finish())
}

fn fingerprint_catalog(catalog: &ObservedCatalog) -> String {
    let mut h = DefaultHasher::new();
    format!("{catalog:?}").hash(&mut h);
    format!("{:x}", h.finish())
}
