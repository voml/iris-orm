//! VOS schema helpers for MySQL wire encoding.

use std::collections::HashSet;

use vos::{CanonicalType, SchemaProjection, TypeContractKind};

/// `(table, field)` pairs declared as VOS `uuid` in the schema projection.
pub fn collect_uuid_fields(document: &SchemaProjection) -> HashSet<(String, String)> {
    let mut out = HashSet::new();
    for table in document.types.iter().filter(|item| item.kind == TypeContractKind::Table) {
        let Some(name) = table.canonical_path.last()
        else {
            continue;
        };
        for field in &table.fields {
            if field_is_uuid(&field.canonical_type) {
                out.insert((name.clone(), field.canonical_name.clone()));
            }
        }
    }
    out
}

fn field_is_uuid(ty: &CanonicalType) -> bool {
    match ty {
        CanonicalType::Optional(inner) => field_is_uuid(inner),
        CanonicalType::Named(path) => path.last().is_some_and(|name| name == "uuid"),
        _ => false,
    }
}
