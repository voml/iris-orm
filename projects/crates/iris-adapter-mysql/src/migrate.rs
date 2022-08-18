//! Managed Push for MySQL (private DDL).

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use iris_types::{LogicalChange, LogicalMigrationPlan, ObservedCatalog};
use mysql::{PooledConn, prelude::*};
use vos::{CanonicalType, FieldContract, SchemaProjection, TypeContract, TypeContractKind};

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
pub fn plan_push(document: &SchemaProjection, observed: &ObservedCatalog) -> Result<LogicalMigrationPlan> {
    let target = fingerprint_document(document);
    let mut changes = Vec::new();
    for table in document.types.iter().filter(|item| item.kind == TypeContractKind::Table) {
        let name = table_name(table);
        match observed.table(name) {
            None => {
                changes.push(LogicalChange::CreateTable { vos_table: name.to_owned() });
            }
            Some(obs) => {
                for field in &table.fields {
                    if !obs.columns.iter().any(|c| c.name == field.canonical_name) {
                        changes.push(LogicalChange::AddField { vos_table: name.to_owned(), vos_field: field.canonical_name.clone() });
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
pub fn apply_push(conn: &mut PooledConn, plan: &LogicalMigrationPlan, document: &SchemaProjection) -> Result<PushReport> {
    if plan.destructive {
        return Err(Error::Policy("refusing to apply destructive plan without explicit policy".into()));
    }
    let mut created = Vec::new();
    conn.query_drop("START TRANSACTION")?;
    let apply = (|| -> Result<()> {
        for change in &plan.changes {
            match change {
                LogicalChange::CreateTable { vos_table } => {
                    let table = find_table(document, vos_table)?;
                    let ddl = create_table_sql(table)?;
                    conn.query_drop(ddl)?;
                    created.push(vos_table.clone());
                }
                LogicalChange::AddField { vos_table, vos_field } => {
                    let table = find_table(document, vos_table)?;
                    let field = table
                        .fields
                        .iter()
                        .find(|f| f.canonical_name == *vos_field)
                        .ok_or_else(|| Error::Policy(format!("plan references unknown VOS field `{vos_table}.{vos_field}`")))?;
                    if is_primary(field) {
                        return Err(Error::Policy(format!("refusing AddField for primary key `{vos_table}.{vos_field}` — recreate table")));
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
            Ok(PushReport { plan_id: plan.id.clone(), created_tables: created })
        }
        Err(e) => {
            let _ = conn.query_drop("ROLLBACK");
            Err(e)
        }
    }
}

fn find_table<'a>(document: &'a SchemaProjection, name: &str) -> Result<&'a TypeContract> {
    document
        .types
        .iter()
        .find(|item| item.kind == TypeContractKind::Table && table_name(item) == name)
        .ok_or_else(|| Error::Policy(format!("plan references unknown VOS table `{name}`")))
}

fn create_table_sql(table: &TypeContract) -> Result<String> {
    let name = table_name(table);
    let mut cols = Vec::new();
    let mut pks = Vec::new();
    for field in &table.fields {
        let (sql_ty, not_null) = map_field_type(field)?;
        let mut piece = format!("`{}` {sql_ty}", field.canonical_name);
        if not_null {
            piece.push_str(" NOT NULL");
        }
        if is_primary(field) {
            pks.push(format!("`{}`", field.canonical_name));
        }
        cols.push(piece);
    }
    if pks.is_empty() {
        return Err(Error::Policy(format!("table `{name}` has no primary key -- cannot push")));
    }
    cols.push(format!("PRIMARY KEY ({})", pks.join(", ")));
    Ok(format!("CREATE TABLE IF NOT EXISTS `{name}` ({}) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;", cols.join(", ")))
}

fn add_field_sql(table: &str, field: &FieldContract) -> Result<String> {
    let (sql_ty, not_null) = map_field_type_for_add(field)?;
    let mut piece = format!("`{}` {sql_ty}", field.canonical_name);
    if not_null {
        // Existing rows need a value when adding NOT NULL columns.
        // Prefer VARCHAR over TEXT so DEFAULT is portable across MySQL versions.
        piece.push_str(" NOT NULL");
        piece.push_str(&format!(" DEFAULT {}", default_literal(field)?));
    }
    else {
        piece.push_str(" NULL");
    }
    Ok(format!("ALTER TABLE `{table}` ADD COLUMN {piece};"))
}

/// Same as [`map_field_type`], except non-PK utf8 uses VARCHAR so NOT NULL DEFAULT works.
fn map_field_type_for_add(field: &FieldContract) -> Result<(String, bool)> {
    let (sql_ty, not_null) = map_field_type(field)?;
    let (inner, _) = strip_optional(&field.canonical_type);
    let sql_ty = match inner {
        CanonicalType::Named(path)
            if matches!(
                path.last().map(String::as_str).unwrap_or_default(),
                "utf8" | "utf16" | "decimal" | "d128" | "date" | "time" | "datetime" | "datetimeUtc"
            ) && !is_primary(field) =>
        {
            "VARCHAR(512)".into()
        }
        _ => sql_ty,
    };
    Ok((sql_ty, not_null))
}

fn default_literal(field: &FieldContract) -> Result<String> {
    let (inner, _) = strip_optional(&field.canonical_type);
    match inner {
        CanonicalType::Named(path) => match path.last().map(String::as_str).unwrap_or_default() {
            "bool" | "i8" | "u8" | "i16" | "u16" | "i32" | "u32" | "i64" | "u64" | "f32" | "f64" => Ok("0".into()),
            "utf8" | "utf16" | "decimal" | "d128" | "date" | "time" | "datetime" | "datetimeUtc" | "bytes" => Ok("''".into()),
            "uuid" => Ok("0x00000000000000000000000000000000".into()),
            other => Err(Error::Policy(format!("no AddField default for VOS type {other}"))),
        },
        other => Err(Error::Policy(format!("no AddField default for VOS type {other:?}"))),
    }
}

fn map_field_type(field: &FieldContract) -> Result<(String, bool)> {
    let (inner, optional) = strip_optional(&field.canonical_type);
    let pk = is_primary(field);
    let sql = match inner {
        CanonicalType::Named(path) => match path.last().map(String::as_str).unwrap_or_default() {
            // MySQL BOOLEAN → TINYINT(1) — document as intentional dialect choice.
            "bool" => "TINYINT(1)",
            "i8" | "u8" | "i16" | "u16" => "SMALLINT",
            "i32" | "u32" => "INT",
            "i64" | "u64" => "BIGINT",
            "f32" => "FLOAT",
            "f64" => "DOUBLE",
            "utf8" | "utf16" | "decimal" | "d128" | "date" | "time" | "datetime" | "datetimeUtc" => {
                if pk {
                    // utf8mb4 indexable PK (InnoDB max index prefix for VARCHAR).
                    "VARCHAR(191)"
                }
                else {
                    "TEXT"
                }
            }
            "uuid" => "BINARY(16)",
            "bytes" => "BLOB",
            other => {
                return Err(Error::Policy(format!("unsupported VOS type for MySQL push: {other}")));
            }
        },
        other => {
            return Err(Error::Policy(format!("unsupported VOS type for MySQL push: {other:?}")));
        }
    };
    Ok((sql.into(), !optional))
}

fn strip_optional(ty: &CanonicalType) -> (&CanonicalType, bool) {
    match ty {
        CanonicalType::Optional(inner) => (inner.as_ref(), true),
        other => (other, false),
    }
}

fn is_primary(field: &FieldContract) -> bool {
    field.attributes.iter().any(|attribute| attribute.name == "primary")
}

fn table_name(table: &TypeContract) -> &str {
    table.canonical_path.last().map(String::as_str).unwrap_or_default()
}

fn fingerprint_document(document: &SchemaProjection) -> String {
    let mut h = DefaultHasher::new();
    format!("{document:?}").hash(&mut h);
    format!("{:x}", h.finish())
}

fn fingerprint_catalog(catalog: &ObservedCatalog) -> String {
    let mut h = DefaultHasher::new();
    format!("{catalog:?}").hash(&mut h);
    format!("{:x}", h.finish())
}
