//! Rust Dejavu template context (presentation-only; no VOS parsing).

use serde::Serialize;
use serde_json::Value;

use crate::{Error, FieldModel, GenerationModel, Result, TableModel};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RustFieldView {
    name: String,
    rust_ty: String,
    optional: bool,
    where_ty: String,
    from_row_expr: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RustTableView {
    name: String,
    snake_name: String,
    primary_key: String,
    scalar_fields: Vec<RustFieldView>,
    synthesize_fns: String,
    row_write_fn: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RustUuidField {
    table: String,
    field: String,
}

#[derive(Debug, Clone, Serialize)]
struct RustTemplateContext {
    generator_version: String,
    schema_fingerprint: String,
    tables: Vec<TableModel>,
    tables_view: Vec<RustTableView>,
    uuid_fields: Vec<RustUuidField>,
}

impl GenerationModel {
    /// JSON context for Rust `.dejavu` templates.
    pub fn rust_template_context(&self) -> Result<Value> {
        Ok(serde_json::to_value(build_rust_context(self)?).expect("RustTemplateContext serializes"))
    }
}

pub(crate) fn build_rust_table_views(model: &GenerationModel) -> Result<Vec<RustTableView>> {
    model
        .tables
        .iter()
        .map(build_rust_table_view)
        .collect()
}

fn build_rust_context(model: &GenerationModel) -> Result<RustTemplateContext> {
    let tables_view = build_rust_table_views(model)?;
    let uuid_fields = model
        .tables
        .iter()
        .flat_map(|table| {
            table.fields.iter().filter(|f| f.is_uuid).map(|field| RustUuidField {
                table: table.name.clone(),
                field: field.name.clone(),
            })
        })
        .collect();

    Ok(RustTemplateContext {
        generator_version: model.generator_version.clone(),
        schema_fingerprint: model.schema_fingerprint.clone(),
        tables: model.tables.clone(),
        tables_view,
        uuid_fields,
    })
}

fn build_rust_table_view(table: &TableModel) -> Result<RustTableView> {
    let pk = primary_key(table)?;
    let scalar_fields: Vec<RustFieldView> = table
        .fields
        .iter()
        .filter(|f| f.reference_target.is_none())
        .map(|field| {
            let extract = value_extract_expr(field);
            let from_row_expr = if field.optional {
                extract
            } else {
                format!("{extract}?")
            };
            RustFieldView {
                name: field.name.clone(),
                rust_ty: field.rust_ty.clone(),
                optional: field.optional,
                where_ty: where_field_ty(field).to_string(),
                from_row_expr,
            }
        })
        .collect();

    for field in &table.fields {
        if field.reference_target.is_some() {
            return Err(Error::UnsupportedType(format!(
                "rust client from_row does not support reference field `{}`",
                field.name
            )));
        }
    }

    Ok(RustTableView {
        name: table.name.clone(),
        snake_name: to_snake(&table.name),
        primary_key: pk.to_string(),
        scalar_fields,
        synthesize_fns: synthesize_fns(table),
        row_write_fn: row_write_fn(table, pk),
    })
}

fn primary_key(table: &TableModel) -> Result<&str> {
    table
        .fields
        .iter()
        .find(|f| f.primary)
        .map(|f| f.name.as_str())
        .ok_or_else(|| {
            Error::Vos(format!(
                "table `{}` has no primary key for Rust CRUD",
                table.name
            ))
        })
}

fn scalar_base(rust_ty: &str) -> &str {
    rust_ty.trim_start_matches("Option<").trim_end_matches('>')
}

fn value_extract_expr(field: &FieldModel) -> String {
    let base = scalar_base(&field.rust_ty);
    match base {
        "bool" => format!("__iris_value_bool(row, \"{}\")", field.name),
        "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" => format!(
            "__iris_value_i64(row, \"{}\").map(|v| v as {base})",
            field.name
        ),
        _ => format!("__iris_value_str(row, \"{}\")", field.name),
    }
}

fn where_field_ty(field: &FieldModel) -> &'static str {
    match scalar_base(&field.rust_ty) {
        "bool" => "Option<bool>",
        "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" => "Option<i64>",
        _ => "Option<String>",
    }
}

fn emit_pred_build(table: &TableModel) -> String {
    let mut pred_build = String::new();
    for field in &table.fields {
        if field.reference_target.is_some() {
            continue;
        }
        match scalar_base(&field.rust_ty) {
            "bool" => {
                pred_build.push_str(&format!(
                    r#"        if let Some(v) = where_.{fname} {{
            if v {{
                preds.push("x.{fname}".to_string());
            }} else {{
                preds.push("x.{fname} == false".to_string());
            }}
        }}
"#,
                    fname = field.name,
                ));
            }
            "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" => {
                pred_build.push_str(&format!(
                    r#"        if let Some(v) = where_.{fname} {{
            preds.push(format!("x.{fname} == {{v}}"));
        }}
"#,
                    fname = field.name,
                ));
            }
            _ => {
                pred_build.push_str(&format!(
                    r#"        if let Some(ref v) = where_.{fname} {{
            preds.push(format!("x.{fname} == \"{{}}\"", escape_vos_str(v)));
        }}
"#,
                    fname = field.name,
                ));
            }
        }
    }
    pred_build
}

fn synthesize_fns(table: &TableModel) -> String {
    let name = &table.name;
    let pred_build = emit_pred_build(table);
    format!(
        r#"    fn synthesize_find_many(where_: &{name}Where) -> String {{
        let mut preds: Vec<String> = Vec::new();
{pred_build}        if preds.is_empty() {{
            format!("{name}.collect()")
        }} else {{
            format!("{name}.filter(x => {{}}).collect()", preds.join(" && "))
        }}
    }}

    fn synthesize_find_unique(where_: &{name}Where) -> String {{
        let mut preds: Vec<String> = Vec::new();
{pred_build}        if preds.is_empty() {{
            format!("{name}.take(1).collect()")
        }} else {{
            format!(
                "{name}.filter(x => {{}}).take(1).collect()",
                preds.join(" && ")
            )
        }}
    }}
"#,
        name = name,
        pred_build = pred_build,
    )
}

fn row_write_fn(table: &TableModel, pk: &str) -> String {
    let mut fields = String::new();
    for field in &table.fields {
        if field.reference_target.is_some() {
            continue;
        }
        let base = scalar_base(&field.rust_ty);
        let value_expr = if field.optional {
            match base {
                "bool" => format!(
                    r#"            ("{fname}".into(), match &row.{fname} {{
                Some(v) => Value::Bool(*v),
                None => Value::Null,
            }}),"#,
                    fname = field.name,
                ),
                "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" => format!(
                    r#"            ("{fname}".into(), match &row.{fname} {{
                Some(v) => Value::Int((*v) as i64),
                None => Value::Null,
            }}),"#,
                    fname = field.name,
                ),
                _ => format!(
                    r#"            ("{fname}".into(), match &row.{fname} {{
                Some(v) => Value::Str(v.clone()),
                None => Value::Null,
            }}),"#,
                    fname = field.name,
                ),
            }
        } else {
            match base {
                "bool" => format!(
                    r#"            ("{fname}".into(), Value::Bool(row.{fname})),"#,
                    fname = field.name,
                ),
                "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" => format!(
                    r#"            ("{fname}".into(), Value::Int(row.{fname} as i64)),"#,
                    fname = field.name,
                ),
                _ => format!(
                    r#"            ("{fname}".into(), Value::Str(row.{fname}.clone())),"#,
                    fname = field.name,
                ),
            }
        };
        fields.push_str(&value_expr);
        fields.push('\n');
    }
    format!(
        r#"    fn to_row_write(row: &{name}) -> RowWrite {{
        RowWrite {{
            table: "{name}".into(),
            primary_key: "{pk}".into(),
            fields: BTreeMap::from([
{fields}            ]),
        }}
    }}
"#,
        name = table.name,
        pk = pk,
        fields = fields,
    )
}

fn to_snake(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}
