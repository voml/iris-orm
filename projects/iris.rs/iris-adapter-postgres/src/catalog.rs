//! PostgreSQL catalog inspect, adopt, and drift.

use iris_types::{
    DriftReport, FieldMapping, MappingManifest, MappingQuality, ObservedCatalog, ObservedColumn,
    ObservedTable, TableMapping,
};
use postgres::Client;
use vos::ast::{Document, Item, TypeExpr};

use crate::outbox;
use crate::{ADAPTER_VERSION, BACKEND_ID, Result};

pub(crate) fn inspect_catalog(client: &mut Client) -> Result<ObservedCatalog> {
    let rows = client.query(
        "SELECT table_name
         FROM information_schema.tables
         WHERE table_schema = 'public' AND table_type = 'BASE TABLE'
         ORDER BY table_name",
        &[],
    )?;
    let mut tables = Vec::new();
    for row in rows {
        let name: String = row.get(0);
        if outbox::is_meta_table(&name) {
            continue;
        }
        let columns = inspect_table(client, &name)?;
        tables.push(ObservedTable { name, columns });
    }
    Ok(ObservedCatalog {
        backend_id: BACKEND_ID.into(),
        tables,
    })
}

fn inspect_table(client: &mut Client, table: &str) -> Result<Vec<ObservedColumn>> {
    let rows = client.query(
        "SELECT
            c.column_name,
            c.udt_name,
            c.is_nullable,
            EXISTS (
                SELECT 1
                FROM information_schema.table_constraints tc
                JOIN information_schema.key_column_usage kcu
                  ON tc.constraint_name = kcu.constraint_name
                 AND tc.table_schema = kcu.table_schema
                WHERE tc.table_schema = 'public'
                  AND tc.table_name = c.table_name
                  AND tc.constraint_type = 'PRIMARY KEY'
                  AND kcu.column_name = c.column_name
            ) AS is_pk
         FROM information_schema.columns c
         WHERE c.table_schema = 'public' AND c.table_name = $1
         ORDER BY c.ordinal_position",
        &[&table],
    )?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let nullable: String = row.get(2);
            ObservedColumn {
                name: row.get(0),
                type_name: row.get(1),
                nullable: nullable.eq_ignore_ascii_case("YES"),
                primary_key: row.get(3),
            }
        })
        .collect())
}

/// Build a reviewable adopt mapping; never invents VOS business names from SQL.
pub fn adopt_plan(document: &Document, catalog: &ObservedCatalog) -> MappingManifest {
    let mut tables = Vec::new();
    for item in &document.items {
        let Item::Table(table) = item else {
            continue;
        };
        let Some(observed) = catalog.table(&table.name) else {
            tables.push(TableMapping {
                vos_table: table.name.clone(),
                physical_table: table.name.clone(),
                fields: Vec::new(),
                blockers: vec![format!(
                    "physical table `{}` not found in observed catalog",
                    table.name
                )],
            });
            continue;
        };

        let mut blockers = Vec::new();
        let pk_count = observed.columns.iter().filter(|c| c.primary_key).count();
        if pk_count == 0 {
            blockers.push(format!(
                "table `{}` has no primary key --?adopt blocked",
                observed.name
            ));
        }

        let mut fields = Vec::new();
        for field in &table.fields {
            let vos_type = type_label(&field.ty);
            match observed.columns.iter().find(|c| c.name == field.name) {
                Some(col) => {
                    let (quality, note) = classify_type(&vos_type, &col.type_name);
                    if quality == MappingQuality::LossyBlocked {
                        blockers.push(format!(
                            "field `{}`.`{}`: cannot map VOS `{vos_type}` to physical `{}` without waiver",
                            table.name, field.name, col.type_name
                        ));
                    }
                    fields.push(FieldMapping {
                        vos_field: field.name.clone(),
                        physical_column: col.name.clone(),
                        vos_type,
                        physical_type: col.type_name.clone(),
                        quality,
                        note,
                    });
                }
                None => {
                    blockers.push(format!(
                        "VOS field `{}`.`{}` has no physical column --?will not auto-invent",
                        table.name, field.name
                    ));
                }
            }
        }

        tables.push(TableMapping {
            vos_table: table.name.clone(),
            physical_table: observed.name.clone(),
            fields,
            blockers,
        });
    }

    MappingManifest {
        adapter_id: BACKEND_ID.into(),
        adapter_version: ADAPTER_VERSION.into(),
        tables,
    }
}

pub(crate) fn drift_report(document: &Document, catalog: &ObservedCatalog) -> DriftReport {
    let vos_tables: Vec<String> = document
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Table(t) => Some(t.name.clone()),
            _ => None,
        })
        .collect();
    let physical: Vec<String> = catalog.tables.iter().map(|t| t.name.clone()).collect();

    let missing_physical_tables = vos_tables
        .iter()
        .filter(|t| !physical.iter().any(|p| p == *t))
        .cloned()
        .collect();
    let extra_physical_tables = physical
        .iter()
        .filter(|p| !vos_tables.iter().any(|t| t == *p))
        .cloned()
        .collect();

    let mut field_mismatches = Vec::new();
    for item in &document.items {
        let Item::Table(table) = item else {
            continue;
        };
        let Some(obs) = catalog.table(&table.name) else {
            continue;
        };
        for field in &table.fields {
            match obs.columns.iter().find(|c| c.name == field.name) {
                None => field_mismatches
                    .push(format!("{}.{} missing physically", table.name, field.name)),
                Some(col) => {
                    let vos_type = type_label(&field.ty);
                    if classify_type(&vos_type, &col.type_name).0 == MappingQuality::LossyBlocked {
                        field_mismatches.push(format!(
                            "{}.{} type drift VOS `{vos_type}` vs `{}`",
                            table.name, field.name, col.type_name
                        ));
                    }
                }
            }
        }
    }

    DriftReport {
        missing_physical_tables,
        extra_physical_tables,
        field_mismatches,
    }
}

fn type_label(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Builtin(b) => format!("{b:?}").to_ascii_lowercase(),
        TypeExpr::Named(n) => n.clone(),
        TypeExpr::Optional(inner) => format!("{}?", type_label(inner)),
        TypeExpr::List(inner) => format!("[{}]", type_label(inner)),
        TypeExpr::Reference(inner) => format!("&{}", type_label(inner)),
        TypeExpr::Vector { dim } => format!("vector<{dim}>"),
        TypeExpr::File => "file".into(),
        _ => "unknown".into(),
    }
}

/// PostgreSQL-specific type classification (udt_name style).
pub fn classify_type(vos_type: &str, physical: &str) -> (MappingQuality, Option<String>) {
    let p = physical.to_ascii_lowercase();
    let v = vos_type.to_ascii_lowercase();
    if (v.contains("utf8") || v.contains("string"))
        && (p == "text" || p == "varchar" || p == "bpchar" || p == "name" || p == "citext")
    {
        return (MappingQuality::Exact, None);
    }
    if v.contains("bool") && (p == "bool" || p == "boolean") {
        return (MappingQuality::Exact, None);
    }
    if (v.contains("i64") || v.contains("u64") || v.contains("i32") || v.contains("u32"))
        && (p == "int8" || p == "int4" || p == "int2" || p == "bigint" || p == "integer")
    {
        return (MappingQuality::Exact, None);
    }
    if v.contains("uuid") && p == "uuid" {
        return (MappingQuality::Exact, None);
    }
    if (v.contains("f64") || v.contains("f32"))
        && (p == "float8" || p == "float4" || p == "double precision" || p == "real")
    {
        return (MappingQuality::Exact, None);
    }
    if v.contains("bytes") && (p == "bytea") {
        return (MappingQuality::Exact, None);
    }
    if v.contains("vector") || v.contains("bytes") {
        return (
            MappingQuality::LossyBlocked,
            Some("complex payload types need an explicit waiver".into()),
        );
    }
    (
        MappingQuality::LossyBlocked,
        Some(format!(
            "no safe default map for `{vos_type}` ???`{physical}`"
        )),
    )
}
