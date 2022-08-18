//! Managed Push for PostgreSQL (private DDL).

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use iris_types::{LogicalChange, LogicalMigrationPlan, ObservedCatalog};
use postgres::Client;
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

/// Build a non-destructive create plan for tables missing physically.
pub fn plan_push(document: &SchemaProjection, observed: &ObservedCatalog) -> Result<LogicalMigrationPlan> {
    let target = fingerprint_document(document);
    let mut changes = Vec::new();
    for table in document.types.iter().filter(|item| item.kind == TypeContractKind::Table) {
        let name = table_name(table);
        if observed.table(name).is_none() {
            changes.push(LogicalChange::CreateTable { vos_table: name.to_owned() });
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

/// Apply a reviewed logical plan by emitting private PostgreSQL DDL.
pub fn apply_push(client: &mut Client, plan: &LogicalMigrationPlan, document: &SchemaProjection) -> Result<PushReport> {
    if plan.destructive {
        return Err(Error::Policy("refusing to apply destructive plan without explicit policy".into()));
    }
    let mut created = Vec::new();
    client.batch_execute("BEGIN")?;
    let apply = (|| -> Result<()> {
        for change in &plan.changes {
            match change {
                LogicalChange::CreateTable { vos_table } => {
                    let table = document
                        .types
                        .iter()
                        .find(|item| item.kind == TypeContractKind::Table && table_name(item) == vos_table)
                        .ok_or_else(|| Error::Policy(format!("plan references unknown VOS table `{vos_table}`")))?;
                    let ddl = create_table_sql(table)?;
                    client.batch_execute(&ddl)?;
                    created.push(vos_table.clone());
                }
                LogicalChange::AddField { .. } => {
                    return Err(Error::Policy("AddField apply is not implemented in Phase 4 slice".into()));
                }
            }
        }
        Ok(())
    })();
    match apply {
        Ok(()) => {
            client.batch_execute("COMMIT")?;
            Ok(PushReport { plan_id: plan.id.clone(), created_tables: created })
        }
        Err(e) => {
            let _ = client.batch_execute("ROLLBACK");
            Err(e)
        }
    }
}

fn create_table_sql(table: &TypeContract) -> Result<String> {
    let name = table_name(table);
    let mut cols = Vec::new();
    let mut pks = Vec::new();
    for field in &table.fields {
        let (sql_ty, not_null) = map_field_type(field)?;
        let mut piece = format!("\"{}\" {sql_ty}", field.canonical_name);
        if not_null {
            piece.push_str(" NOT NULL");
        }
        if is_primary(field) {
            pks.push(format!("\"{}\"", field.canonical_name));
        }
        cols.push(piece);
    }
    if pks.is_empty() {
        return Err(Error::Policy(format!("table `{name}` has no primary key --?cannot push")));
    }
    cols.push(format!("PRIMARY KEY ({})", pks.join(", ")));
    Ok(format!("CREATE TABLE IF NOT EXISTS \"{name}\" ({});", cols.join(", ")))
}

fn map_field_type(field: &FieldContract) -> Result<(String, bool)> {
    let (inner, optional) = strip_optional(&field.canonical_type);
    let sql = match inner {
        CanonicalType::Named(path) => match path.last().map(String::as_str).unwrap_or_default() {
            "bool" => "BOOLEAN",
            "i8" | "i16" | "u8" | "u16" => "SMALLINT",
            "i32" | "u32" => "INTEGER",
            "i64" | "u64" => "BIGINT",
            "f32" => "REAL",
            "f64" => "DOUBLE PRECISION",
            "utf8" | "utf16" | "decimal" | "d128" | "date" | "time" | "datetime" | "datetimeUtc" => "TEXT",
            "uuid" => "UUID",
            "bytes" => "BYTEA",
            other => {
                return Err(Error::Policy(format!("unsupported VOS type for PostgreSQL push: {other}")));
            }
        },
        other => {
            return Err(Error::Policy(format!("unsupported VOS type for PostgreSQL push: {other:?}")));
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
