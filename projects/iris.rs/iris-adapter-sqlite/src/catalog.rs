//! Catalog inspect, adopt planning, and drift (no SQL in public values).

use iris_types::{
    DriftReport, FieldMapping, MappingManifest, MappingQuality, ObservedCatalog, ObservedColumn,
    ObservedTable, TableMapping,
};
use rusqlite::Connection;
use vos::ast::{Document, Item, TypeExpr};

use crate::outbox;
use crate::{ADAPTER_VERSION, BACKEND_ID, Result};

pub(crate) fn inspect_catalog(conn: &Connection) -> Result<ObservedCatalog> {
    let mut tables = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let names = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for name in names {
        if outbox::is_meta_table(&name) {
            continue;
        }
        let columns = inspect_table(conn, &name)?;
        tables.push(ObservedTable { name, columns });
    }
    Ok(ObservedCatalog {
        backend_id: BACKEND_ID.into(),
        tables,
    })
}

fn inspect_table(conn: &Connection, table: &str) -> Result<Vec<ObservedColumn>> {
    // PRAGMA table_info is the SQLite catalog API; kept private to this module.
    // Quote the identifier so names with punctuation never become command text.
    let escaped = table.replace('"', "\"\"");
    let pragma = format!("PRAGMA table_info(\"{escaped}\")");
    let mut stmt = conn.prepare(&pragma)?;
    let cols = stmt
        .query_map([], |row| {
            Ok(ObservedColumn {
                name: row.get::<_, String>(1)?,
                type_name: row.get::<_, String>(2)?,
                nullable: row.get::<_, i64>(3)? == 0,
                primary_key: row.get::<_, i64>(5)? > 0,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(cols)
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

fn classify_type(vos_type: &str, physical: &str) -> (MappingQuality, Option<String>) {
    let p = physical.to_ascii_uppercase();
    let v = vos_type.to_ascii_lowercase();
    if (v.contains("utf8") || v.contains("string"))
        && (p.contains("TEXT") || p.contains("CHAR") || p.contains("CLOB"))
    {
        return (MappingQuality::Exact, None);
    }
    if v.contains("bool") && (p.contains("INT") || p == "BOOLEAN") {
        return (
            MappingQuality::Compatible,
            Some("bool stored as INTEGER 0/1".into()),
        );
    }
    if (v.contains("i64") || v.contains("u64") || v.contains("i32") || v.contains("u32"))
        && p.contains("INT")
    {
        return (MappingQuality::Exact, None);
    }
    if v.contains("uuid") && (p.contains("TEXT") || p.contains("BLOB")) {
        return (
            MappingQuality::Compatible,
            Some("uuid stored as TEXT".into()),
        );
    }
    if (v.contains("f64") || v.contains("f32"))
        && (p.contains("REAL") || p.contains("FLOA") || p.contains("DOUB"))
    {
        return (MappingQuality::Exact, None);
    }
    // Unknown / vector / bytes without explicit mapping ???block on adopt.
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
