//! TypeScript Dejavu template context (presentation-only; no VOS parsing).

use std::collections::HashSet;

use serde::Serialize;
use serde_json::Value;

use crate::{Error, FieldModel, GenerationModel, MacroModel, Result, TableModel, carrier, resolve_operation_carrier};

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
    /// Durable VOS operation identity the delegate must pass to the executor.
    operation_id: String,
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
    /// Per-entity map onto the generated `<Entity>WhereInput`, so nested
    /// `WherePathFor` reuses each field's faithful label-dispatched surface.
    where_map_lines: String,
    models_import_lines: String,
    refs_import_lines: String,
    input_import_names: String,
    ref_type_imports: String,
    wire_map_lines: String,
    value_carrier_lines: String,
    reference_target_lines: String,
    tables_view: Vec<TsTableView>,
    macros_view: Vec<TsMacroView>,
    has_macros: bool,
    /// Pre-computed per-entity wire-name map inlined into synthesize.ts
    /// so the runtime no longer imports IRIS_FIELD_WIRE_NAMES from metadata.
    synthesize_wire_map_lines: String,
    /// Pre-computed per-entity reference-target map inlined into synthesize.ts
    /// so the runtime no longer imports IRIS_REFERENCE_TARGETS from metadata.
    synthesize_reference_target_lines: String,
    /// Per-entity field → operator allow-list inlined into synthesize.ts, so
    /// the runtime lowers filters by the faithful VOS label instead of
    /// accepting any operator key a generic object happens to carry.
    synthesize_filter_ops_lines: String,
}

impl GenerationModel {
    /// JSON context for TypeScript `.dejavu` templates.
    pub fn typescript_template_context(&self) -> Result<Value> {
        self.validate_operation_surface()?;
        validate_ts_naming(self)?;
        Ok(serde_json::to_value(build_ts_context(self)?).expect("TsTemplateContext serializes"))
    }
}

fn build_ts_context(model: &GenerationModel) -> Result<TsTemplateContext> {
    let entity_names: HashSet<&str> = model.tables.iter().map(|t| t.name.as_str()).collect();
    let declared: HashSet<String> = model.tables.iter().map(|table| table.name.clone()).collect();
    let entity_union = if model.tables.is_empty() {
        "never".into()
    }
    else {
        model.tables.iter().map(|t| format!("\"{}\"", t.name)).collect::<Vec<_>>().join(" | ")
    };
    let entity_map_lines = model.tables.iter().map(|t| format!("    {}: {};", t.name, t.name)).collect::<Vec<_>>().join("\n");
    let where_map_lines =
        model.tables.iter().map(|t| format!("    {}: {}WhereInput;", t.name, t.name)).collect::<Vec<_>>().join("\n");
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
                format!("{}UpdateArgs", table.name),
                format!("{}GetPayload", table.name),
            ]
        })
        .collect::<Vec<_>>()
        .join(", ");
    let needed = reference_targets(model);
    let ref_type_imports = needed.iter().flat_map(|name| [format!("{name}Id"), format!("{name}Reference")]).collect::<Vec<_>>().join(", ");

    let mut tables_view = Vec::with_capacity(model.tables.len());
    for table in &model.tables {
        tables_view.push(build_table_view(table, &entity_names)?);
    }

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

    let value_carrier_lines = model
        .tables
        .iter()
        .map(|table| {
            let fields = table
                .fields
                .iter()
                .map(|field| format!("{}: \"{}\"", field_ts_name(field), carrier::base_vos_type(&field.vos_type)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("    {}: {{ {} }},", table.name, fields)
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

    // Declared macro operations are the only macro delegates. Identity comes
    // from `OperationModel.operation_id`, never from the author name, so a
    // generated call and a direct call resolve the same declared operation.
    let mut macros_view = Vec::with_capacity(model.macros.len());
    for operation in model.operations.iter().filter(|operation| operation.kind == "macro") {
        let ts_return = match operation.return_type.as_deref() {
            Some(ret) => resolve_operation_carrier(&operation.name, ret, &declared).map(|carrier| carrier.ts_type)?,
            None => "void".to_owned(),
        };
        let params = operation
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
        macros_view.push(TsMacroView {
            name: operation.name.clone(),
            operation_id: operation.operation_id.clone(),
            uses_execute: ts_return == "void",
            ts_return,
            params,
            param_signature,
            param_names,
        });
    }

    // Pre-computed maps that get inlined directly into synthesize.ts so the
    // runtime no longer pulls them from metadata.ts. Same shape as
    // wire_map_lines / reference_target_lines — just consumed by a different
    // template, keeping metadata.ts's copies for node/browser/cloudflare.
    let synthesize_wire_map_lines = wire_map_lines.clone();
    let synthesize_reference_target_lines = reference_target_lines.clone();
    let synthesize_filter_ops_lines = model
        .tables
        .iter()
        .map(|table| {
            let fields = table
                .fields
                .iter()
                .filter(|field| field.reference_target.is_none())
                .map(|field| {
                    let (_name, ops) = vos_filter_surface(&field.vos_type).ok_or_else(|| Error::UnsupportedFilterType {
                        entity: table.name.clone(),
                        field: field.name.clone(),
                        vos_type: field.vos_type.clone(),
                    })?;
                    let mut allowed: Vec<&str> = ops.to_vec();
                    if field.optional {
                        allowed.extend(["isNull", "isNotNull"]);
                    }
                    let ops = allowed.iter().map(|op| format!("\"{op}\"")).collect::<Vec<_>>().join(", ");
                    Ok(format!("{}: [{ops}]", field_ts_name(field)))
                })
                .collect::<Result<Vec<_>>>()?
                .join(", ");
            Ok(format!("    {}: {{ {fields} }},", table.name))
        })
        .collect::<Result<Vec<_>>>()?
        .join("\n");
    let has_macros = !macros_view.is_empty();

    Ok(TsTemplateContext {
        generator_version: model.generator_version.clone(),
        schema_fingerprint: model.schema_fingerprint.clone(),
        tables: model.tables.clone(),
        macros: model.macros.clone(),
        entity_union,
        entity_map_lines,
        where_map_lines,
        models_import_lines,
        refs_import_lines,
        input_import_names,
        ref_type_imports,
        wire_map_lines,
        value_carrier_lines,
        reference_target_lines,
        tables_view,
        macros_view,
        has_macros,
        synthesize_wire_map_lines,
        synthesize_reference_target_lines,
        synthesize_filter_ops_lines,
    })
}

fn build_table_view(table: &TableModel, entity_names: &HashSet<&str>) -> Result<TsTableView> {
    let pk = primary_key_field(table);
    let pk_name = pk.map(|f| f.name.as_str()).unwrap_or("id");
    let pk_vos = pk.map(|f| f.vos_type.as_str()).unwrap_or("utf8");
    let pk_ts_scalar = carrier::vos_type_to_ts(pk_vos)?;

    let primary_fields: Vec<_> = table.fields.iter().filter(|f| f.primary).collect();
    let unique_where_lines = if primary_fields.is_empty() {
        format!("    id?: {}Id;", table.name)
    }
    else {
        primary_fields.iter().map(|field| format!("    {}: {}Id;", field_ts_name(field), table.name)).collect::<Vec<_>>().join("\n")
    };

    let mut fields = Vec::with_capacity(table.fields.len());
    for field in &table.fields {
        fields.push(TsFieldView {
            name: field.name.clone(),
            ts_name: field_ts_name(field),
            ts_model_type: model_field_type(table, field)?,
            where_line: emit_where_field(table, field, entity_names)?,
            select_line: emit_select_field(field, entity_names),
            create_line: emit_create_field(table, field, entity_names)?,
            patch_line: emit_patch_field(table, field, entity_names)?,
        });
    }

    Ok(TsTableView {
        name: table.name.clone(),
        camel_name: camel_case(&table.name),
        pk_name: pk_name.into(),
        pk_ts_name: pk.map(field_ts_name).unwrap_or_else(|| "id".into()),
        pk_vos_type: pk_vos.into(),
        pk_ts_scalar,
        fields,
        unique_where_lines,
    })
}

fn camel_case(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
    }
}

/// Typed filter surface of one VOS label: the generated filter type name and
/// the operator allow-list the runtime lowers for fields carrying that label
/// (`None` = the label has no filter).
///
/// Returning `Option` keeps generation honest: a label with no declared filter
/// type hard-fails in `emit_where_field` instead of silently becoming
/// `StringFilter`. The operator list is the single source for the runtime map
/// inlined into `_internal/synthesize.ts`, so the lowered surface and the
/// authored surface can never drift apart.
fn vos_filter_surface(vos_type: &str) -> Option<(&'static str, &'static [&'static str])> {
    let base = carrier::base_vos_type(vos_type);
    Some(match base {
        "bool" => ("BooleanFilter", &["eq", "not"]),
        "i64" | "u64" => ("BigIntFilter", &["eq", "not", "gt", "gte", "lt", "lte"]),
        "i8" | "i16" | "i32" | "u8" | "u16" | "u32" | "f32" | "f64" => ("NumberFilter", &["eq", "not", "gt", "gte", "lt", "lte"]),
        "utf8" | "utf16" => ("StringFilter", &["eq", "not", "contains", "startsWith", "endsWith"]),
        "uuid" => ("UuidFilter", &["eq", "not"]),
        "decimal" => ("DecimalFilter", &["eq", "not", "gt", "gte", "lt", "lte"]),
        "date" | "time" | "datetimeUtc" => ("DateTimeFilter", &["eq", "not", "gt", "gte", "lt", "lte"]),
        "bytes" => ("BytesFilter", &["eq", "not"]),
        _ => return None,
    })
}

fn model_field_type(table: &TableModel, field: &FieldModel) -> Result<String> {
    if let Some(ref target) = field.reference_target {
        let reference = format!("{target}Reference");
        return Ok(if field.optional { format!("{reference} | null") } else { reference });
    }
    if field.primary {
        return Ok(format!("{}Id", table.name));
    }
    let base = carrier::vos_type_to_ts(&field.vos_type)?;
    Ok(if field.optional { format!("{base} | null") } else { base })
}

fn scalar_filter_type(table: &TableModel, field: &FieldModel) -> Result<String> {
    if field.primary { Ok(format!("{}Id", table.name)) } else { carrier::vos_type_to_ts(&field.vos_type) }
}

fn nullable_where_suffix(optional: bool) -> &'static str {
    if optional { " | { readonly isNull: true } | { readonly isNotNull: true }" } else { "" }
}

fn patch_value_type(table: &TableModel, field: &FieldModel, entity_names: &HashSet<&str>) -> Result<String> {
    if let Some(ref target) = field.reference_target
        && entity_names.contains(target.as_str())
    {
        let inner = format!("{}RefInput", target);
        return Ok(if field.optional { format!("NullablePatchValue<{inner}>") } else { format!("PatchValue<{inner}>") });
    }
    let inner = if field.primary {
        format!("{}Id", table.name)
    }
    else {
        let model_ty = model_field_type(table, field)?;
        strip_null_union(&model_ty)
    };
    Ok(if field.optional { format!("NullablePatchValue<{inner}>") } else { format!("PatchValue<{inner}>") })
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

fn emit_where_field(table: &TableModel, field: &FieldModel, entity_names: &HashSet<&str>) -> Result<Option<String>> {
    if let Some(ref target) = field.reference_target {
        if entity_names.contains(target.as_str()) {
            return Ok(Some(format!(
                "    {}?: WherePathFor<\"{}\"> | {{ readonly is: WherePathFor<\"{}\"> }};",
                field_ts_name(field),
                target,
                target
            )));
        }
        return Ok(None);
    }
    let scalar = scalar_filter_type(table, field)?;
    let (filter, _ops) = vos_filter_surface(&field.vos_type).ok_or_else(|| Error::UnsupportedFilterType {
        entity: table.name.clone(),
        field: field.name.clone(),
        vos_type: field.vos_type.clone(),
    })?;
    let nullable = nullable_where_suffix(field.optional);
    Ok(Some(format!("    {}?: {scalar} | {filter}{nullable};", field_ts_name(field), scalar = scalar, filter = filter, nullable = nullable,)))
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

fn emit_create_field(table: &TableModel, field: &FieldModel, entity_names: &HashSet<&str>) -> Result<Option<String>> {
    if let Some(ref target) = field.reference_target {
        if !entity_names.contains(target.as_str()) {
            return Ok(None);
        }
        let req = if field.optional { "?" } else { "" };
        return Ok(Some(format!("    {}{}: {}RefInput;", field_ts_name(field), req, target)));
    }
    let ty = model_field_type(table, field)?;
    Ok(Some(format!("    {}{}: {};", field_ts_name(field), if field.optional { "?" } else { "" }, ty)))
}

fn emit_patch_field(table: &TableModel, field: &FieldModel, entity_names: &HashSet<&str>) -> Result<Option<String>> {
    if field.primary {
        return Ok(None);
    }
    if let Some(ref target) = field.reference_target
        && !entity_names.contains(target.as_str())
    {
        return Ok(None);
    }
    Ok(Some(format!("    {}?: {};", field_ts_name(field), patch_value_type(table, field, entity_names)?)))
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
