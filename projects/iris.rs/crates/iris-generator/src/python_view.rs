//! Python Dejavu template context (presentation-only; no VOS parsing).
//!
//! Python field names are the VOS wire names themselves (snake case is already
//! the Python convention), so this view carries no wire-name translation table.
//! Only the label-dispatched filter surface, reference targets and vector
//! dimensions need inlining, and those come from the same helpers the
//! TypeScript view uses.

use serde::Serialize;
use serde_json::Value;

use crate::{FieldModel, GenerationModel, Result, TableModel, carrier, ts_view};

#[derive(Debug, Clone, Serialize)]
struct PyTableView {
    name: String,
    /// Lowercase entity key of `client.tables.<accessor>`.
    accessor: String,
    /// Union of `vector` field names for the `find_near` `field` argument.
    vector_field_union: String,
    has_vector: bool,
    /// Row-model attribute annotations, in field order.
    model_fields: String,
    /// Row-model `__init__` body: copy the fields the row actually carries.
    model_assignments: String,
    where_fields: String,
    select_fields: String,
    create_fields: String,
    patch_fields: String,
    primary_field: String,
    ref_fields: String,
}

#[derive(Debug, Clone, Serialize)]
struct PyTemplateContext {
    generator_version: String,
    schema_fingerprint: String,
    /// VOS schema text as a Python string literal for generated metadata.
    schema_source_literal: String,
    tables_view: Vec<PyTableView>,
    has_operations: bool,
    /// `{entity: {field: [ops...]}}` Python literal inlined into synthesize.
    filter_ops_literal: String,
    /// `{entity: {field: "Target"}}` Python literal inlined into synthesize.
    reference_targets_literal: String,
    /// `{entity: {field: 3 or None}}` Python literal inlined into synthesize.
    vector_fields_literal: String,
}

impl GenerationModel {
    /// JSON context for Python `.dejavu` templates.
    pub fn python_template_context(&self) -> Result<Value> {
        self.validate_operation_surface()?;
        Ok(serde_json::to_value(build_py_context(self)?).expect("PyTemplateContext serializes"))
    }
}

fn build_py_context(model: &GenerationModel) -> Result<PyTemplateContext> {
    let tables_view = model.tables.iter().map(build_table_view).collect::<Result<Vec<_>>>()?;

    let filter_ops_literal = model
        .tables
        .iter()
        .map(|table| {
            let fields = table
                .fields
                .iter()
                .filter(|field| field.reference_target.is_none())
                .filter_map(|field| {
                    if carrier::base_vos_type(&field.vos_type) == "vector" {
                        return None;
                    }
                    let (_name, ops) = ts_view::vos_filter_surface(&field.vos_type)?;
                    let mut allowed: Vec<&str> = ops.to_vec();
                    if field.optional {
                        allowed.extend(["is_null", "is_not_null"]);
                    }
                    Some(Ok(format!("{}: [{}]", py_string(&field.name), allowed.iter().map(|op| py_string(op)).collect::<Vec<_>>().join(", "))))
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(format!("{}: {{{}}}", py_string(&table.name), fields.join(", ")))
        })
        .collect::<Result<Vec<_>>>()?
        .join(", ");

    let reference_targets_literal = model
        .tables
        .iter()
        .map(|table| {
            let refs = table
                .fields
                .iter()
                .filter_map(|field| field.reference_target.as_ref().map(|target| format!("{}: {}", py_string(&field.name), py_string(target))))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}: {{{}}}", py_string(&table.name), refs)
        })
        .collect::<Vec<_>>()
        .join(", ");

    let vector_fields_literal = model
        .tables
        .iter()
        .filter(|table| table.fields.iter().any(|field| carrier::base_vos_type(&field.vos_type) == "vector"))
        .map(|table| {
            let fields = table
                .fields
                .iter()
                .filter(|field| carrier::base_vos_type(&field.vos_type) == "vector")
                .map(|field| {
                    let dimension = ts_view::vector_dimension(field).ok().flatten();
                    let rendered = dimension.map(|value| value.to_string()).unwrap_or_else(|| "None".into());
                    format!("{}: {}", py_string(&field.name), rendered)
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}: {{{}}}", py_string(&table.name), fields)
        })
        .collect::<Vec<_>>()
        .join(", ");

    Ok(PyTemplateContext {
        generator_version: model.generator_version.clone(),
        schema_fingerprint: model.schema_fingerprint.clone(),
        schema_source_literal: serde_json::to_string(&model.schema_source).expect("schema source serializes"),
        tables_view,
        has_operations: !model.operations.is_empty(),
        filter_ops_literal,
        reference_targets_literal,
        vector_fields_literal,
    })
}

fn build_table_view(table: &TableModel) -> Result<PyTableView> {
    let mut model_fields = Vec::with_capacity(table.fields.len());
    let mut model_assignments = Vec::with_capacity(table.fields.len());
    let mut where_fields = Vec::new();
    let mut select_fields = Vec::with_capacity(table.fields.len());
    let mut create_fields = Vec::with_capacity(table.fields.len());
    let mut patch_fields = Vec::new();
    let mut vector_names = Vec::new();

    for field in &table.fields {
        let base = carrier::base_vos_type(&field.vos_type);
        let value_type = python_value_type(field)?;
        model_fields.push(format!("    {}: {}", field.name, value_type));
        model_assignments.push(format!(
            "        if {} in row:\n            self.{} = row[{}]",
            py_string(&field.name),
            field.name,
            py_string(&field.name)
        ));

        if let Some(target) = field.reference_target.as_ref() {
            where_fields.push(format!("    {}: NotRequired[{}WhereInput]", field.name, target));
            select_fields.push(format!("    {}: NotRequired[bool]", field.name));
            create_fields.push(format!(
                "    {}: {}",
                field.name,
                if field.optional { format!("NotRequired[{}RefInput | str]", target) } else { format!("{}RefInput | str", target) }
            ));
        }
        else if base == "vector" {
            vector_names.push(field.name.clone());
            if field.optional {
                create_fields.push(format!("    {}: NotRequired[Sequence[float]]", field.name));
            }
            else {
                create_fields.push(format!("    {}: Sequence[float]", field.name));
            }
        }
        else {
            let filter = ts_view::vos_filter_surface(&field.vos_type).map(|(name, _)| name);
            let where_type = match filter {
                Some(filter) => {
                    let nullity = if field.optional { " | NoneFilter" } else { "" };
                    format!("{} | {}Filter{}", value_type, py_filter_stem(filter), nullity)
                }
                None => value_type.clone(),
            };
            where_fields.push(format!("    {}: NotRequired[{}]", field.name, where_type));
            select_fields.push(format!("    {}: NotRequired[bool]", field.name));
            create_fields.push(format!(
                "    {}: {}",
                field.name,
                if field.optional { format!("NotRequired[{value_type}]") } else { value_type.clone() }
            ));
            if !field.primary {
                let patch_value =
                    if field.optional { format!("NullablePatchValue[{}]", value_type) } else { format!("PatchValue[{}]", value_type) };
                patch_fields.push(format!("    {}: NotRequired[{}]", field.name, patch_value));
            }
        }
    }

    Ok(PyTableView {
        name: table.name.clone(),
        accessor: table.name.to_ascii_lowercase(),
        vector_field_union: vector_names.iter().map(|name| format!("\"{name}\"")).collect::<Vec<_>>().join(" | "),
        has_vector: !vector_names.is_empty(),
        model_fields: model_fields.join("\n"),
        model_assignments: model_assignments.join("\n"),
        where_fields: body(where_fields),
        select_fields: body(select_fields),
        create_fields: body(create_fields),
        patch_fields: body(patch_fields),
        primary_field: primary_field_name(table),
        ref_fields: reference_fields(table)?,
    })
}

/// Class body of a generated `TypedDict`. An empty body would be a syntax error.
fn body(lines: Vec<String>) -> String {
    if lines.is_empty() {
        "    pass".into()
    } else {
        lines.join("\n")
    }
}

/// `{pk: value}` write shape of a `&T` reference input.
fn reference_fields(table: &TableModel) -> Result<String> {
    match table.fields.iter().find(|field| field.primary).or_else(|| table.fields.first()) {
        Some(field) => Ok(format!("    {}: {}", field.name, python_value_type(field)?)),
        None => Ok("    pass".into()),
    }
}

fn primary_field_name(table: &TableModel) -> String {
    match table.fields.iter().find(|field| field.primary).or_else(|| table.fields.first()) {
        Some(field) => format!("\"{}\"", field.name),
        None => "\"id\"".into(),
    }
}

/// Python row/carrier type of one field.
///
/// Tagged wire carriers (`bytes`, `vector`, `file`) surface as the canonical
/// `{"$yydb": …}` mapping the binding returns, so the generated row type never
/// promises a Python object the transport does not produce.
fn python_value_type(field: &FieldModel) -> Result<String> {
    let base = carrier::base_vos_type(&field.vos_type);
    let inner = match base {
        "bool" => "bool",
        "i8" | "i16" | "i32" | "u8" | "u16" | "u32" | "i64" | "u64" => "int",
        "f32" | "f64" => "float",
        "utf8" | "utf16" | "uuid" | "decimal" | "date" | "time" | "datetimeUtc" => "str",
        "bytes" | "vector" | "file" => "Mapping[str, Any]",
        other => {
            if other.contains("::") || other.chars().next().is_some_and(|character| character.is_ascii_uppercase()) {
                other
            }
            else {
                return Err(crate::Error::UnknownVosLabel(other.to_owned()));
            }
        }
    };
    Ok(if field.optional { format!("{inner} | None") } else { inner.to_owned() })
}

/// Python filter stem of the TypeScript filter type (`StringFilter` → `String`).
fn py_filter_stem(ts_filter: &str) -> &str {
    ts_filter.strip_suffix("Filter").unwrap_or(ts_filter)
}

fn py_string(value: &str) -> String {
    serde_json::to_string(value).expect("string serializes")
}
