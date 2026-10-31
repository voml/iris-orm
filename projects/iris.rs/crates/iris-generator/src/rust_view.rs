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
    where_fields: Vec<RustFieldView>,
    synthesize_fns: String,
    row_write_fn: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RustUuidField {
    table: String,
    field: String,
}

/// Rust adapter binding for one datasource kind whose adapter surface has converged.
#[derive(Debug, Clone, Serialize)]
struct RustDatasourceBindingView {
    kind: String,
    adapter_crate: &'static str,
    source_type: &'static str,
    conn_type: &'static str,
    result_alias: &'static str,
}

/// Resolve the Rust client adapter binding for a datasource kind.
///
/// `mysql` and `yydb` have converged adapter surfaces (connection handoff,
/// `_on` variants, `with_rollback`). `sqlite` / `postgres` / `redis` / `yyds` /
/// `reference` fail closed until their adapter surfaces converge on the same shape.
fn rust_datasource_binding(kind: &str) -> Result<RustDatasourceBindingView> {
    match kind {
        "mysql" => Ok(RustDatasourceBindingView {
            kind: kind.to_owned(),
            adapter_crate: "iris_adapter_mysql",
            source_type: "MysqlSource",
            conn_type: "PooledConn",
            result_alias: "MysqlResult",
        }),
        "yydb" => Ok(RustDatasourceBindingView {
            kind: kind.to_owned(),
            adapter_crate: "iris_connector_yydb",
            source_type: "YydbSource",
            conn_type: "Connection",
            result_alias: "YydbResult",
        }),
        other => Err(Error::UnsupportedDatasource(other.to_owned())),
    }
}

#[derive(Debug, Clone, Serialize)]
struct RustTemplateContext {
    generator_version: String,
    schema_fingerprint: String,
    datasource: RustDatasourceBindingView,
    tables: Vec<TableModel>,
    tables_view: Vec<RustTableView>,
    uuid_fields: Vec<RustUuidField>,
}

impl GenerationModel {
    /// JSON context for Rust `.dejavu` templates, bound to one datasource kind.
    pub fn rust_template_context(&self, datasource: &str) -> Result<Value> {
        Ok(serde_json::to_value(build_rust_context(self, datasource)?).expect("RustTemplateContext serializes"))
    }
}

pub(crate) fn build_rust_table_views(model: &GenerationModel) -> Result<Vec<RustTableView>> {
    model.tables.iter().map(build_rust_table_view).collect()
}

fn build_rust_context(model: &GenerationModel, datasource: &str) -> Result<RustTemplateContext> {
    let datasource_binding = rust_datasource_binding(datasource)?;
    let tables_view = build_rust_table_views(model)?;
    let uuid_fields = model
        .tables
        .iter()
        .flat_map(|table| {
            table.fields.iter().filter(|f| f.is_uuid).map(|field| RustUuidField { table: table.name.clone(), field: field.name.clone() })
        })
        .collect();

    Ok(RustTemplateContext {
        generator_version: model.generator_version.clone(),
        schema_fingerprint: model.schema_fingerprint.clone(),
        datasource: datasource_binding,
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
            let from_row_expr = if field.optional { extract } else { format!("{extract}?") };
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
            return Err(Error::UnsupportedType(format!("rust client from_row does not support reference field `{}`", field.name)));
        }
    }

    let where_fields = scalar_fields.iter().filter(|f| !is_payload_base(scalar_base(&f.rust_ty))).cloned().collect();

    Ok(RustTableView {
        name: table.name.clone(),
        snake_name: to_snake(&table.name),
        primary_key: pk.to_string(),
        scalar_fields,
        where_fields,
        synthesize_fns: synthesize_fns(table),
        row_write_fn: row_write_fn(table, pk)?,
    })
}

fn primary_key(table: &TableModel) -> Result<&str> {
    table
        .fields
        .iter()
        .find(|f| f.primary)
        .map(|f| f.name.as_str())
        .ok_or_else(|| Error::Vos(format!("table `{}` has no primary key for Rust CRUD", table.name)))
}

fn scalar_base(rust_ty: &str) -> &str {
    rust_ty
        .strip_prefix("Option<")
        .and_then(|rest| rest.strip_suffix('>'))
        .unwrap_or(rust_ty)
}

/// Payload carriers (`bytes` / `vector` / `file`) have no VOS literal text form,
/// so they never join the synthesized `Where` equality predicates.
fn is_payload_base(base: &str) -> bool {
    matches!(base, "Vec<u8>" | "Vec<f32>" | "iris::FileDescriptor")
}

/// Dimension parsed from the field's `vector<N>` / `vector<N>?` VOS label.
fn vector_dimension(field: &FieldModel) -> Result<u32> {
    let label = field.vos_type.strip_suffix('?').unwrap_or(&field.vos_type);
    let inner = label
        .strip_prefix("vector<")
        .and_then(|rest| rest.strip_suffix('>'))
        .ok_or_else(|| Error::UnsupportedType(format!("field `{}` has malformed vector type `{}`", field.name, field.vos_type)))?;
    let dimension: u32 = inner
        .parse()
        .map_err(|_| Error::UnsupportedType(format!("vector dimension `{inner}` is not a positive integer")))?;
    if dimension == 0 {
        return Err(Error::UnsupportedType("vector dimension must be greater than zero".into()));
    }
    Ok(dimension)
}

fn value_extract_expr(field: &FieldModel) -> String {
    let base = scalar_base(&field.rust_ty);
    match base {
        "bool" => format!("__iris_value_bool(row, \"{}\")", field.name),
        "u64" => format!("__iris_value_u64(row, \"{}\")", field.name),
        "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" => {
            format!("__iris_value_i64(row, \"{}\").map(|v| v as {base})", field.name)
        }
        "f64" => format!("__iris_value_f64(row, \"{}\")", field.name),
        "f32" => format!("__iris_value_f64(row, \"{}\").map(|v| v as f32)", field.name),
        "Vec<u8>" => format!("__iris_value_bytes(row, \"{}\")", field.name),
        "Vec<f32>" => format!("__iris_value_vector(row, \"{}\")", field.name),
        "iris::FileDescriptor" => format!("__iris_value_file(row, \"{}\")", field.name),
        _ => format!("__iris_value_str(row, \"{}\")", field.name),
    }
}

fn where_field_ty(field: &FieldModel) -> &'static str {
    match scalar_base(&field.rust_ty) {
        "bool" => "Option<bool>",
        "u64" => "Option<u64>",
        "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" => "Option<i64>",
        "f32" | "f64" => "Option<f64>",
        _ => "Option<String>",
    }
}

fn emit_pred_build(table: &TableModel) -> String {
    let mut pred_build = String::new();
    for field in &table.fields {
        if field.reference_target.is_some() || is_payload_base(scalar_base(&field.rust_ty)) {
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
            "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "f32" | "f64" => {
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

fn row_write_fn(table: &TableModel, pk: &str) -> Result<String> {
    let mut fields = String::new();
    for field in &table.fields {
        if field.reference_target.is_some() {
            continue;
        }
        let base = scalar_base(&field.rust_ty);
        let fname = field.name.as_str();
        let value_expr = match base {
            "Vec<u8>" => {
                if field.optional {
                    format!(
                        r#"            ("{fname}".into(), match &row.{fname} {{
                Some(v) => Value::Bytes(v.clone()),
                None => Value::Null,
            }}),"#,
                    )
                }
                else {
                    format!(r#"            ("{fname}".into(), Value::Bytes(row.{fname}.clone())),"#)
                }
            }
            "Vec<f32>" => {
                let dimension = vector_dimension(field)?;
                let policy = format!("vector `{fname}` does not match the declared dimension {dimension}");
                if field.optional {
                    format!(
                        r#"            ("{fname}".into(), match &row.{fname} {{
                Some(v) => Value::Vector(iris::VectorValue::new({dimension}u32, v.clone()).ok_or_else(|| super::errors::Error::Policy("{policy}".into()))?),
                None => Value::Null,
            }}),"#,
                    )
                }
                else {
                    format!(
                        r#"            ("{fname}".into(), Value::Vector(iris::VectorValue::new({dimension}u32, row.{fname}.clone()).ok_or_else(|| super::errors::Error::Policy("{policy}".into()))?)),"#,
                    )
                }
            }
            "iris::FileDescriptor" => {
                if field.optional {
                    format!(
                        r#"            ("{fname}".into(), match &row.{fname} {{
                Some(v) => Value::File(v.clone()),
                None => Value::Null,
            }}),"#,
                    )
                }
                else {
                    format!(r#"            ("{fname}".into(), Value::File(row.{fname}.clone())),"#)
                }
            }
            _ => {
                if field.optional {
                    match base {
                        "bool" => format!(
                            r#"            ("{fname}".into(), match &row.{fname} {{
                Some(v) => Value::Bool(*v),
                None => Value::Null,
            }}),"#,
                        ),
                        "u64" => format!(
                            r#"            ("{fname}".into(), match &row.{fname} {{
                Some(v) => Value::UInt(*v),
                None => Value::Null,
            }}),"#,
                        ),
                        base @ ("f32" | "f64") => {
                            let convert = if base == "f32" { "f64::from(*v)" } else { "*v" };
                            format!(
                                r#"            ("{fname}".into(), match &row.{fname} {{
                Some(v) => {{
                    if !v.is_finite() {{
                        return Err(super::errors::Error::Policy("field `{fname}` is a non-finite VOS `{base}`".into()));
                    }}
                    Value::Float({convert})
                }}
                None => Value::Null,
            }}),"#,
                            )
                        }
                        "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" => format!(
                            r#"            ("{fname}".into(), match &row.{fname} {{
                Some(v) => Value::Int((*v) as i64),
                None => Value::Null,
            }}),"#,
                        ),
                        _ => format!(
                            r#"            ("{fname}".into(), match &row.{fname} {{
                Some(v) => Value::Str(v.clone()),
                None => Value::Null,
            }}),"#,
                        ),
                    }
                }
                else {
                    match base {
                        "bool" => format!(r#"            ("{fname}".into(), Value::Bool(row.{fname})),"#),
                        "u64" => format!(r#"            ("{fname}".into(), Value::UInt(row.{fname})),"#),
                        base @ ("f32" | "f64") => {
                            let convert = if base == "f32" { format!("f64::from(row.{fname})") } else { format!("row.{fname}") };
                            format!(
                                r#"            ("{fname}".into(), {{
                if !row.{fname}.is_finite() {{
                    return Err(super::errors::Error::Policy("field `{fname}` is a non-finite VOS `{base}`".into()));
                }}
                Value::Float({convert})
            }}),"#,
                            )
                        }
                        "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" => {
                            format!(r#"            ("{fname}".into(), Value::Int(row.{fname} as i64)),"#)
                        }
                        _ => format!(r#"            ("{fname}".into(), Value::Str(row.{fname}.clone())),"#),
                    }
                }
            }
        };
        fields.push_str(&value_expr);
        fields.push('\n');
    }
    Ok(format!(
        r#"    fn to_row_write(row: &{name}) -> super::errors::Result<RowWrite> {{
        Ok(RowWrite {{
            table: "{name}".into(),
            primary_key: "{pk}".into(),
            fields: BTreeMap::from([
{fields}            ]),
        }})
    }}
"#,
        name = table.name,
        pk = pk,
        fields = fields,
    ))
}

fn to_snake(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        }
        else {
            out.push(c);
        }
    }
    out
}
