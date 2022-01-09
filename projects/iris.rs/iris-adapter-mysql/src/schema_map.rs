//! VOS schema helpers for MySQL wire encoding.

use std::collections::HashSet;

use vos::ast::{BuiltinType, Document, Item, TypeExpr};

/// `(table, field)` pairs declared as VOS `uuid` in the schema document.
pub fn collect_uuid_fields(document: &Document) -> HashSet<(String, String)> {
    let mut out = HashSet::new();
    for item in &document.items {
        let Item::Table(table) = item else {
            continue;
        };
        for field in &table.fields {
            if field_is_uuid(&field.ty) {
                out.insert((table.name.clone(), field.name.clone()));
            }
        }
    }
    out
}

fn field_is_uuid(ty: &TypeExpr) -> bool {
    match ty {
        TypeExpr::Optional(inner) => field_is_uuid(inner),
        TypeExpr::Builtin(BuiltinType::Uuid) => true,
        _ => false,
    }
}
