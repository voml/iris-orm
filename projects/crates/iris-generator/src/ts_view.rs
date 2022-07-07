//! TypeScript Dejavu template context (presentation-only; no VOS parsing).

use std::collections::HashSet;

use serde::Serialize;
use serde_json::Value;

use crate::{Error, FieldModel, GenerationModel, MacroModel, Result, TableModel};

#[derive(Debug, Clone, Serialize)]
struct TsFieldView {
    name: String,
    ts_name: String,
    ts_model_type: String,
    where_line: Option<String>,
    select_line: Option<String>,
    create_line: Option<String>,
    patch_line: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct TsTableView {
    name: String,
    camel_name: String,
    pk_name: String,
    pk_ts_name: String,
    pk_vos_type: String,
    pk_ts_scalar: String,
    fields: Vec<TsFieldView>,
    unique_where_lines: String,
}

#[derive(Debug, Clone, Serialize)]
struct TsMacroParamView {
    name: String,
    ts_type: String,
}

#[derive(Debug, Clone, Serialize)]
struct TsMacroView {
    name: String,
    ts_return: String,
    uses_execute: bool,
    params: Vec<TsMacroParamView>,
    param_signature: String,
    param_names: String,
}

#[derive(Debug, Clone, Serialize)]
struct TsTemplateContext {
    generator_version: String,
    schema_fingerprint: String,
    tables: Vec<TableModel>,
    macros: Vec<MacroModel>,
    entity_union: String,
    entity_map_lines: String,
    models_import_lines: String,
    refs_import_lines: String,
    input_import_names: String,
    ref_type_imports: String,
    wire_map_lines: String,
    reference_target_lines: String,
    d1_plan_lines: String,
    tables_view: Vec<TsTableView>,
    macros_view: Vec<TsMacroView>,
    has_macros: bool,
}

impl GenerationModel {
    /// JSON context for TypeScript `.dejavu` templates.
    pub fn typescript_template_context(&self) -> Result<Value> {
        validate_ts_naming(self)?;
        Ok(serde_json::to_value(build_ts_context(self)).expect("TsTemplateContext serializes"))
    }
}

fn build_ts_context(model: &GenerationModel) -> TsTemplateContext {
    let entity_names: HashSet<&str> = model.tables.iter().map(|t| t.name.as_str()).collect();
    let entity_union = if model.tables.is_empty() {
        "never".into()
    }
    else {
        model.tables.iter().map(|t| format!("\"{}\"", t.name)).collect::<Vec<_>>().join(" | ")
    };
    let entity_map_lines = model.tables.iter().map(|t| format!("    {}: {};", t.name, t.name)).collect::<Vec<_>>().join("\n");
    let models_import_lines = model.tables.iter().map(|t| format!("    {},", t.name)).collect::<Vec<_>>().join("\n");
    let refs_import_lines =
        model.tables.iter().flat_map(|t| [format!("    {}Id,", t.name), format!("    {}RefInput,", t.name)]).collect::<Vec<_>>().join("\n");
    let input_import_names = model
        .tables
        .iter()
        .flat_map(|table| {
            [
                format!("{}FindManyArgs", table.name),
                format!("{}FindFirstArgs", table.name),
                format!("{}FindUniqueArgs", table.name),
                format!("{}CreateArgs", table.name),
                format!("{}DeleteArgs", table.name),
                format!("{}GetPayload", table.name),
            ]
        })
        .collect::<Vec<_>>()
        .join(", ");
    let needed = reference_targets(model);
    let ref_type_imports = needed.iter().flat_map(|name| [format!("{name}Id"), format!("{name}Reference")]).collect::<Vec<_>>().join(", ");

    let tables_view = model.tables.iter().map(|table| build_table_view(table, &entity_names)).collect();

    let wire_map_lines = model
        .tables
        .iter()
        .map(|table| {
            let fields = table
                .fields
                .iter()
                .map(|field| format!("{}: \"{}\"", field_ts_name(field), field.name.replace('\\', "\\\\").replace('"', "\\\"")))
                .collect::<Vec<_>>()
                .join(", ");
            format!("    {}: {{ {} }},", table.name, fields)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let d1_plan_lines = model
        .tables
        .iter()
        .flat_map(|table| {
            let cols = table.fields.iter().map(|field| field.name.as_str()).collect::<Vec<_>>().join(", ");
            let entity = table.name.as_str();
            let insert_fields = d1_insert_fields(table, &entity_names);
            let mut lines = vec![
                format!("    \"{entity}.findMany\": {{ sql: \"SELECT {cols} FROM {entity}\", mode: \"read\" }},"),
                format!(
                    "    \"{entity}.findMany@take\": {{ sql: \"SELECT {cols} FROM {entity} LIMIT ?\", mode: \"read\", paramOrder: [\"take\"] }},"
                ),
                format!("    \"{entity}.findFirst\": {{ sql: \"SELECT {cols} FROM {entity} LIMIT 1\", mode: \"read\" }},"),
            ];
            for field in table.fields.iter().filter(|field| field.primary) {
                let wire = field.name.as_str();
                let param_key = format!("p_{wire}");
                lines.push(format!(
                    "    \"{entity}.findUnique@{param_key}\": {{ sql: \"SELECT {cols} FROM {entity} WHERE {wire} = ? LIMIT 1\", mode: \"read\", paramOrder: [\"{param_key}\"] }},"
                ));
                lines.push(format!(
                    "    \"{entity}.delete@{param_key}\": {{ sql: \"DELETE FROM {entity} WHERE {wire} = ?\", mode: \"write\", paramOrder: [\"{param_key}\"] }},"
                ));
            }
            for field in &table.fields {
                let wire = field.name.as_str();
                let param_key = format!("p_{wire}");
                let base = field.vos_type.trim_end_matches('?').trim_start_matches('&');
                if matches!(base, "bool" | "utf8" | "uuid" | "decimal" | "datetime") {
                    lines.push(format!(
                        "    \"{entity}.findMany@{param_key}\": {{ sql: \"SELECT {cols} FROM {entity} WHERE {wire} = ?\", mode: \"read\", paramOrder: [\"{param_key}\"] }},"
                    ));
                    lines.push(format!(
                        "    \"{entity}.findMany@{param_key},take\": {{ sql: \"SELECT {cols} FROM {entity} WHERE {wire} = ? LIMIT ?\", mode: \"read\", paramOrder: [\"{param_key}\", \"take\"] }},"
                    ));
                }
            }
            push_d1_create_plans(&mut lines, entity, &insert_fields);
            lines
        })
        .collect::<Vec<_>>()
        .join("\n");

    let reference_target_lines = model
        .tables
        .iter()
        .map(|table| {
            let refs = table
                .fields
                .iter()
                .filter_map(|field| {
                    field
                        .reference_target
                        .as_ref()
                        .filter(|target| entity_names.contains(target.as_str()))
                        .map(|target| format!("{}: \"{}\"", field_ts_name(field), target.replace('\\', "\\\\").replace('"', "\\\"")))
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("    {}: {{ {} }},", table.name, refs)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let macros_view = model
        .macros
        .iter()
        .map(|macro_def| {
            let ts_return = macro_return_ts(&macro_def.return_type);
            let params = macro_def
                .params
                .iter()
                .map(|param| TsMacroParamView { name: param.name.clone(), ts_type: param.ts_type.clone() })
                .collect::<Vec<_>>();
            let param_signature = if params.is_empty() {
                String::new()
            }
            else {
                params.iter().map(|param| format!("{}: {}", param.name, param.ts_type)).collect::<Vec<_>>().join(", ")
            };
            let param_names = params.iter().map(|param| param.name.clone()).collect::<Vec<_>>().join(", ");
            TsMacroView { name: macro_def.name.clone(), uses_execute: ts_return == "void", ts_return, params, param_signature, param_names }
        })
        .collect();

    TsTemplateContext {
        generator_version: model.generator_version.clone(),
        schema_fingerprint: model.schema_fingerprint.clone(),
        tables: model.tables.clone(),
        macros: model.macros.clone(),
        entity_union,
        entity_map_lines,
        models_import_lines,
        refs_import_lines,
        input_import_names,
        ref_type_imports,
        wire_map_lines,
        reference_target_lines,
        d1_plan_lines,
        tables_view,
        macros_view,
        has_macros: !model.macros.is_empty(),
    }
}

fn build_table_view(table: &TableModel, entity_names: &HashSet<&str>) -> TsTableView {
    let pk = primary_key_field(table);
    let pk_name = pk.map(|f| f.name.as_str()).unwrap_or("id");
    let pk_vos = pk.map(|f| f.vos_type.as_str()).unwrap_or("utf8");
    let pk_ts_scalar = scalar_base_type(pk_vos).to_string();

    let primary_fields: Vec<_> = table.fields.iter().filter(|f| f.primary).collect();
    let unique_where_lines = if primary_fields.is_empty() {
        format!("    id?: {}Id;", table.name)
    }
    else {
        primary_fields.iter().map(|field| format!("    {}: {}Id;", field_ts_name(field), table.name)).collect::<Vec<_>>().join("\n")
    };

    let fields = table
        .fields
        .iter()
        .map(|field| TsFieldView {
            name: field.name.clone(),
            ts_name: field_ts_name(field),
            ts_model_type: model_field_type(table, field),
            where_line: emit_where_field(table, field, entity_names),
            select_line: emit_select_field(field, entity_names),
            create_line: emit_create_field(table, field, entity_names),
            patch_line: emit_patch_field(table, field, entity_names),
        })
        .collect();

    TsTableView {
        name: table.name.clone(),
        camel_name: camel_case(&table.name),
        pk_name: pk_name.into(),
        pk_ts_name: pk.map(field_ts_name).unwrap_or_else(|| "id".into()),
        pk_vos_type: pk_vos.into(),
        pk_ts_scalar,
        fields,
        unique_where_lines,
    }
}

fn camel_case(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
    }
}

fn scalar_base_type(vos_type: &str) -> &'static str {
    let base = vos_type.trim_end_matches('?').trim_start_matches('&');
    match base {
        "bool" => "boolean",
        "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "f32" | "f64" => "number",
        _ => "string",
    }
}

fn vos_filter_name(vos_type: &str) -> &'static str {
    let base = vos_type.trim_end_matches('?').trim_start_matches('&');
    match base {
        "bool" => "BooleanFilter",
        "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "f32" | "f64" => "NumberFilter",
        "utf8" | "utf16" => "StringFilter",
        "uuid" => "UuidFilter",
        "decimal" => "DecimalFilter",
        "datetime" => "DateTimeFilter",
        "bytes" => "BytesFilter",
        _ => "StringFilter",
    }
}

fn model_field_type(table: &TableModel, field: &FieldModel) -> String {
    if let Some(ref target) = field.reference_target {
        let reference = format!("{target}Reference");
        return if field.optional { format!("{reference} | null") } else { reference };
    }
    if field.primary {
        return format!("{}Id", table.name);
    }
    let base = scalar_base_type(&field.vos_type);
    if field.optional { format!("{base} | null") } else { base.into() }
}

fn scalar_filter_type(table: &TableModel, field: &FieldModel) -> String {
    if field.primary { format!("{}Id", table.name) } else { scalar_base_type(&field.vos_type).into() }
}

fn nullable_where_suffix(optional: bool) -> &'static str {
    if optional { " | { readonly isNull: true } | { readonly isNotNull: true }" } else { "" }
}

fn patch_value_type(table: &TableModel, field: &FieldModel, entity_names: &HashSet<&str>) -> String {
    if let Some(ref target) = field.reference_target {
        if entity_names.contains(target.as_str()) {
            let inner = format!("{}RefInput", target);
            return if field.optional { format!("NullablePatchValue<{inner}>") } else { format!("PatchValue<{inner}>") };
        }
    }
    let inner = if field.primary {
        format!("{}Id", table.name)
    }
    else {
        let model_ty = model_field_type(table, field);
        strip_null_union(&model_ty)
    };
    if field.optional { format!("NullablePatchValue<{inner}>") } else { format!("PatchValue<{inner}>") }
}

fn strip_null_union(ty: &str) -> String {
    ty.replace(" | null", "")
}

fn primary_key_field(table: &TableModel) -> Option<&FieldModel> {
    table.fields.iter().find(|f| f.primary).or(table.fields.first())
}

fn reference_targets(model: &GenerationModel) -> HashSet<String> {
    let mut targets = HashSet::new();
    for table in &model.tables {
        targets.insert(table.name.clone());
        for field in &table.fields {
            if let Some(ref target) = field.reference_target {
                targets.insert(target.clone());
            }
        }
    }
    targets
}

fn emit_where_field(table: &TableModel, field: &FieldModel, entity_names: &HashSet<&str>) -> Option<String> {
    if let Some(ref target) = field.reference_target {
        if entity_names.contains(target.as_str()) {
            return Some(format!(
                "    {}?: WherePathFor<\"{}\"> | {{ readonly is: WherePathFor<\"{}\"> }};",
                field_ts_name(field),
                target,
                target
            ));
        }
        return None;
    }
    let scalar = scalar_filter_type(table, field);
    let filter = vos_filter_name(&field.vos_type);
    let nullable = nullable_where_suffix(field.optional);
    Some(format!("    {}?: {scalar} | {filter}{nullable};", field_ts_name(field), scalar = scalar, filter = filter, nullable = nullable,))
}

fn emit_select_field(field: &FieldModel, entity_names: &HashSet<&str>) -> Option<String> {
    if let Some(ref target) = field.reference_target {
        if entity_names.contains(target.as_str()) {
            return Some(format!("    {}?: boolean | {{ readonly select: SelectPathFor<\"{}\"> }};", field_ts_name(field), target));
        }
        return None;
    }
    Some(format!("    {}?: boolean;", field_ts_name(field)))
}

fn emit_create_field(table: &TableModel, field: &FieldModel, entity_names: &HashSet<&str>) -> Option<String> {
    if let Some(ref target) = field.reference_target {
        if !entity_names.contains(target.as_str()) {
            return None;
        }
        let req = if field.optional { "?" } else { "" };
        return Some(format!("    {}{}: {}RefInput;", field_ts_name(field), req, target));
    }
    if field.optional {
        Some(format!("    {}?: {};", field_ts_name(field), model_field_type(table, field)))
    }
    else {
        Some(format!("    {}: {};", field_ts_name(field), model_field_type(table, field)))
    }
}

fn emit_patch_field(table: &TableModel, field: &FieldModel, entity_names: &HashSet<&str>) -> Option<String> {
    if field.primary {
        return None;
    }
    if let Some(ref target) = field.reference_target {
        if !entity_names.contains(target.as_str()) {
            return None;
        }
    }
    Some(format!("    {}?: {};", field_ts_name(field), patch_value_type(table, field, entity_names)))
}

fn macro_return_ts(return_type: &str) -> String {
    let base = return_type.trim_end_matches('?');
    match base {
        "utf8" | "uuid" | "decimal" | "datetime" | "bytes" => "string".into(),
        "bool" => "boolean".into(),
        "unit" => "void".into(),
        "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "f32" | "f64" => "number".into(),
        other if other.contains("::") || other.chars().next().is_some_and(|c| c.is_ascii_uppercase()) => other.into(),
        _ => "unknown".into(),
    }
}

fn field_ts_name(field: &FieldModel) -> String {
    snake_to_camel(&field.name)
}

fn snake_to_camel(name: &str) -> String {
    let parts: Vec<&str> = name.split('_').filter(|segment| !segment.is_empty()).collect();
    if parts.is_empty() {
        return String::new();
    }
    let mut out = parts[0].to_string();
    for part in parts.iter().skip(1) {
        if let Some(first) = part.chars().next() {
            out.push_str(&first.to_uppercase().collect::<String>());
            out.push_str(&part.chars().skip(1).collect::<String>());
        }
    }
    out
}

const MAX_D1_CREATE_SUBSET_FIELDS: usize = 8;

fn d1_insert_field_in_schema(field: &FieldModel, entity_names: &HashSet<&str>) -> bool {
    match &field.reference_target {
        Some(target) => entity_names.contains(target.as_str()),
        None => true,
    }
}

fn d1_insert_fields<'a>(table: &'a TableModel, entity_names: &HashSet<&str>) -> Vec<&'a FieldModel> {
    table.fields.iter().filter(|field| d1_insert_field_in_schema(field, entity_names)).collect()
}

fn push_d1_create_plans(lines: &mut Vec<String>, entity: &str, insert_fields: &[&FieldModel]) {
    if insert_fields.is_empty() || insert_fields.len() > MAX_D1_CREATE_SUBSET_FIELDS {
        return;
    }
    let field_count = insert_fields.len();
    for mask in 1..(1usize << field_count) {
        let subset: Vec<_> = insert_fields
            .iter()
            .enumerate()
            .filter(|(index, _)| mask & (1usize << index) != 0)
            .map(|(_, field)| *field)
            .collect();
        let insert_cols = subset.iter().map(|field| field.name.as_str()).collect::<Vec<_>>().join(", ");
        let placeholders = subset.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let data_param_keys = subset.iter().map(|field| format!("data_{}", field.name)).collect::<Vec<_>>();
        let mut sorted_data_keys = data_param_keys.clone();
        sorted_data_keys.sort();
        let variant_suffix = sorted_data_keys.join(",");
        let param_order = data_param_keys.iter().map(|key| format!("\"{key}\"")).collect::<Vec<_>>().join(", ");
        lines.push(format!(
            "    \"{entity}.create@{variant_suffix}\": {{ sql: \"INSERT INTO {entity} ({insert_cols}) VALUES ({placeholders}) RETURNING {insert_cols}\", mode: \"write-returning\", paramOrder: [{param_order}] }},"
        ));
    }
}

fn validate_ts_naming(model: &GenerationModel) -> Result<()> {
    use std::collections::HashMap;

    for table in &model.tables {
        let mut seen: HashMap<String, String> = HashMap::new();
        for field in &table.fields {
            let ts_name = field_ts_name(field);
            if let Some(wire_a) = seen.get(&ts_name) {
                return Err(Error::NamingCollision { entity: table.name.clone(), wire_a: wire_a.clone(), wire_b: field.name.clone(), ts_name });
            }
            seen.insert(ts_name, field.name.clone());
        }
    }
    Ok(())
}
