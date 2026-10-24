//! Catalog inspect, adopt planning, and drift (no SQL in public values).

use iris_types::{DriftReport, FieldMapping, MappingManifest, MappingQuality, ObservedCatalog, ObservedColumn, ObservedTable, TableMapping};
use vos::{CanonicalType, CanonicalTypeArgument, SchemaProjection, TypeContract, TypeContractKind};

use crate::{ADAPTER_VERSION, BACKEND_ID, Result, outbox, provider::RusqliteProvider};

pub(crate) fn inspect_catalog(provider: &RusqliteProvider) -> Result<ObservedCatalog> {
    let snapshot = provider.inspect_catalog()?;
    let mut tables = Vec::new();
    for table in snapshot.tables {
        if outbox::is_meta_table(&table.name) {
            continue;
        }
        let columns = table
            .columns
            .into_iter()
            .map(|col| ObservedColumn { name: col.name, type_name: col.type_name, nullable: col.nullable, primary_key: col.primary_key })
            .collect();
        tables.push(ObservedTable { name: table.name, columns });
    }
    Ok(ObservedCatalog { backend_id: BACKEND_ID.into(), tables })
}

/// Build a reviewable adopt mapping; never invents VOS business names from SQL.
pub fn adopt_plan(document: &SchemaProjection, catalog: &ObservedCatalog) -> MappingManifest {
    let mut tables = Vec::new();
    for table in document.types.iter().filter(|item| item.kind == TypeContractKind::Table) {
        let name = table_name(table);
        let Some(observed) = catalog.table(name)
        else {
            tables.push(TableMapping {
                vos_table: name.to_owned(),
                physical_table: name.to_owned(),
                fields: Vec::new(),
                blockers: vec![format!("physical table `{name}` not found in observed catalog")],
            });
            continue;
        };

        let mut blockers = Vec::new();
        let pk_count = observed.columns.iter().filter(|c| c.primary_key).count();
        if pk_count == 0 {
            blockers.push(format!("table `{}` has no primary key --?adopt blocked", observed.name));
        }

        let mut fields = Vec::new();
        for field in &table.fields {
            let vos_type = type_label(&field.canonical_type);
            match observed.columns.iter().find(|c| c.name == field.canonical_name) {
                Some(col) => {
                    let (quality, note) = classify_type(&vos_type, &col.type_name);
                    if quality == MappingQuality::LossyBlocked {
                        blockers.push(format!(
                            "field `{}`.`{}`: cannot map VOS `{vos_type}` to physical `{}` without waiver",
                            name, field.canonical_name, col.type_name
                        ));
                    }
                    fields.push(FieldMapping {
                        vos_field: field.canonical_name.clone(),
                        physical_column: col.name.clone(),
                        vos_type,
                        physical_type: col.type_name.clone(),
                        quality,
                        note,
                    });
                }
                None => {
                    blockers.push(format!("VOS field `{}`.`{}` has no physical column --?will not auto-invent", name, field.canonical_name));
                }
            }
        }

        tables.push(TableMapping { vos_table: name.to_owned(), physical_table: observed.name.clone(), fields, blockers });
    }

    MappingManifest { adapter_id: BACKEND_ID.into(), adapter_version: ADAPTER_VERSION.into(), tables }
}

pub(crate) fn drift_report(document: &SchemaProjection, catalog: &ObservedCatalog) -> DriftReport {
    let vos_tables: Vec<String> =
        document.types.iter().filter(|item| item.kind == TypeContractKind::Table).map(|item| table_name(item).to_owned()).collect();
    let physical: Vec<String> = catalog.tables.iter().map(|t| t.name.clone()).collect();

    let missing_physical_tables = vos_tables.iter().filter(|t| !physical.iter().any(|p| p == *t)).cloned().collect();
    let extra_physical_tables = physical.iter().filter(|p| !vos_tables.iter().any(|t| t == *p)).cloned().collect();

    let mut field_mismatches = Vec::new();
    for table in document.types.iter().filter(|item| item.kind == TypeContractKind::Table) {
        let name = table_name(table);
        let Some(obs) = catalog.table(name)
        else {
            continue;
        };
        for field in &table.fields {
            match obs.columns.iter().find(|c| c.name == field.canonical_name) {
                None => field_mismatches.push(format!("{name}.{} missing physically", field.canonical_name)),
                Some(col) => {
                    let vos_type = type_label(&field.canonical_type);
                    if classify_type(&vos_type, &col.type_name).0 == MappingQuality::LossyBlocked {
                        field_mismatches.push(format!("{name}.{} type drift VOS `{vos_type}` vs `{}`", field.canonical_name, col.type_name));
                    }
                }
            }
        }
    }

    DriftReport { missing_physical_tables, extra_physical_tables, field_mismatches }
}

fn table_name(table: &TypeContract) -> &str {
    table.canonical_path.last().map(String::as_str).unwrap_or_default()
}

fn type_label(ty: &CanonicalType) -> String {
    match ty {
        CanonicalType::Named(path) => path.last().cloned().unwrap_or_default(),
        CanonicalType::Optional(inner) => format!("{}?", type_label(inner)),
        CanonicalType::List(inner) => format!("[{}]", type_label(inner)),
        CanonicalType::Reference(inner) => format!("&{}", type_label(inner)),
        CanonicalType::Generic { path, arguments } => {
            let arguments = arguments
                .iter()
                .map(|argument| match argument {
                    CanonicalTypeArgument::Type(value) => type_label(value),
                    CanonicalTypeArgument::Literal(value) => value.clone(),
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}<{arguments}>", path.last().cloned().unwrap_or_default())
        }
    }
}

fn classify_type(vos_type: &str, physical: &str) -> (MappingQuality, Option<String>) {
    let p = physical.to_ascii_uppercase();
    let v = vos_type.to_ascii_lowercase();
    if (v.contains("utf8") || v.contains("string")) && (p.contains("TEXT") || p.contains("CHAR") || p.contains("CLOB")) {
        return (MappingQuality::Exact, None);
    }
    if v.contains("bool") && (p.contains("INT") || p == "BOOLEAN") {
        return (MappingQuality::Compatible, Some("bool stored as INTEGER 0/1".into()));
    }
    if (v.contains("i64") || v.contains("u64") || v.contains("i32") || v.contains("u32")) && p.contains("INT") {
        return (MappingQuality::Exact, None);
    }
    if v.contains("uuid") && (p.contains("TEXT") || p.contains("BLOB")) {
        return (MappingQuality::Compatible, Some("uuid stored as TEXT".into()));
    }
    if (v.contains("f64") || v.contains("f32")) && (p.contains("REAL") || p.contains("FLOA") || p.contains("DOUB")) {
        return (MappingQuality::Exact, None);
    }
    // Unknown / vector / bytes without explicit mapping ???block on adopt.
    if v.contains("vector") || v.contains("bytes") {
        return (MappingQuality::LossyBlocked, Some("complex payload types need an explicit waiver".into()));
    }
    (MappingQuality::LossyBlocked, Some(format!("no safe default map for `{vos_type}` ???`{physical}`")))
}
