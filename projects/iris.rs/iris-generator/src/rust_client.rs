//! Emit thin Rust `Db` / typed CRUD (TS-parity shim; not knife-B identity IR).
//!
//! Generated code targets `MysqlSource` + `Planner`, synthesizes VOS `.filter`
//! pipelines with escaped literals. Pooling stays inside `MysqlSource`; apps see
//! `Db` and, for a unit of work, `Txn` (same method names).

use crate::{Error, FieldModel, GenerationModel, Result, TableModel};

/// Append typed MySQL client + per-table delegates to a domain `mod.rs` body.
pub fn emit_rust_mysql_client(model: &GenerationModel) -> Result<String> {
    let mut out = String::new();
    out.push_str(
        r#"
// --- Thin generated client (TS-parity shim; not knife-B GeneratedCall) ---------
// Prefer these delegates over hand-written VOS strings / RowWrite. Escape hatch:
// `Db::query` / `Db::execute` (aligns with Session::query / db.$query).
// `transaction` / `with_rollback` give a `Txn` with the same CRUD surface;
// the connection is held by Iris — do not call MysqlSource checkout APIs from `f`.

use std::collections::BTreeMap;

use iris::{Planner, Row, RowWrite, Value};
use iris_adapter_mysql::{MysqlSource, Result as MysqlResult};

fn __iris_escape_vos_str(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(ch),
        }
    }
    out
}

fn __iris_value_str(row: &Row, key: &str) -> Option<String> {
    match row.get(key)? {
        Value::Str(s) => Some(s.clone()),
        Value::Null => None,
        other => Some(format!("{other:?}")),
    }
}

fn __iris_value_bool(row: &Row, key: &str) -> Option<bool> {
    match row.get(key)? {
        Value::Bool(b) => Some(*b),
        // MySQL BOOLEAN is TINYINT(1); adapter surfaces Int.
        Value::Int(i) => Some(*i != 0),
        _ => None,
    }
}

fn __iris_value_i64(row: &Row, key: &str) -> Option<i64> {
    match row.get(key)? {
        Value::Int(i) => Some(*i),
        _ => None,
    }
}

"#,
    );

    for table in &model.tables {
        out.push_str(&emit_from_row(table)?);
        out.push_str(&emit_where_struct(table)?);
    }

    out.push_str(
        r#"
/// Generated Iris client bound to a MySQL adapter.
pub struct Db<'a> {
    source: &'a MysqlSource,
    planner: Planner,
}

impl<'a> Db<'a> {
    /// Bind to an open [`MysqlSource`].
    pub fn new(source: &'a MysqlSource) -> Self {
        Self {
            source,
            planner: Planner::new(MysqlSource::capabilities()),
        }
    }

    /// Escape hatch: plan + execute VOS DML (returns rows). Prefer typed delegates.
    pub fn query(&self, source: &str) -> MysqlResult<Vec<Row>> {
        let plan = self
            .planner
            .plan_source(source)
            .map_err(|e| iris_adapter_mysql::Error::Policy(e.to_string()))?;
        self.source.execute_plan(&plan)
    }

    /// Escape hatch: unit-valued / DDL-shaped VOS.
    pub fn execute(&self, source: &str) -> MysqlResult<()> {
        let _ = self.query(source)?;
        Ok(())
    }

    /// Commit-on-success transaction. Use [`Txn`] inside `f` (same CRUD names as [`Db`]).
    pub fn transaction<R>(
        &self,
        f: impl FnOnce(&mut Txn<'_>) -> MysqlResult<R>,
    ) -> MysqlResult<R> {
        self.source.transaction(|conn| {
            let mut txn = Txn {
                source: self.source,
                planner: &self.planner,
                conn,
            };
            f(&mut txn)
        })
    }

    /// Always `ROLLBACK` — shared-DB integration tests (fixtures leave no residue).
    pub fn with_rollback<R>(
        &self,
        f: impl FnOnce(&mut Txn<'_>) -> MysqlResult<R>,
    ) -> MysqlResult<R> {
        self.source.with_rollback(|conn| {
            let mut txn = Txn {
                source: self.source,
                planner: &self.planner,
                conn,
            };
            f(&mut txn)
        })
    }
"#,
    );

    for table in &model.tables {
        let method = to_snake(&table.name);
        out.push_str(&format!(
            r#"
    /// Table delegate for `{name}`.
    pub fn {method}(&self) -> {name}Delegate<'_> {{
        {name}Delegate {{ db: self }}
    }}
"#,
            name = table.name,
            method = method,
        ));
    }

    out.push_str("}\n");

    out.push_str(
        r#"
/// Transaction handle: same CRUD names as [`Db`]; Iris holds one connection for the closure.
pub struct Txn<'a> {
    source: &'a MysqlSource,
    planner: &'a Planner,
    conn: &'a mut iris_adapter_mysql::PooledConn,
}

impl Txn<'_> {
    /// Escape hatch while the transaction connection is held.
    pub fn query(&mut self, source: &str) -> MysqlResult<Vec<Row>> {
        let plan = self
            .planner
            .plan_source(source)
            .map_err(|e| iris_adapter_mysql::Error::Policy(e.to_string()))?;
        self.source.execute_plan_on(self.conn, &plan)
    }

    /// Unit-valued escape hatch while the transaction connection is held.
    pub fn execute(&mut self, source: &str) -> MysqlResult<()> {
        let _ = self.query(source)?;
        Ok(())
    }
"#,
    );

    for table in &model.tables {
        let method = to_snake(&table.name);
        out.push_str(&format!(
            r#"
    /// Table delegate for `{name}` on this transaction.
    pub fn {method}(&mut self) -> {name}TxnDelegate<'_> {{
        {name}TxnDelegate {{
            source: self.source,
            planner: self.planner,
            conn: self.conn,
        }}
    }}
"#,
            name = table.name,
            method = method,
        ));
    }

    out.push_str("}\n");

    for table in &model.tables {
        out.push_str(&emit_db_delegate(table)?);
        out.push_str(&emit_txn_delegate(table)?);
    }

    Ok(out)
}

fn scalar_base(rust_ty: &str) -> &str {
    rust_ty
        .trim_start_matches("Option<")
        .trim_end_matches('>')
}

fn emit_from_row(table: &TableModel) -> Result<String> {
    let mut fields = String::new();
    for field in &table.fields {
        if field.reference_target.is_some() {
            return Err(Error::UnsupportedType(format!(
                "rust client from_row does not support reference field `{}`",
                field.name
            )));
        }
        let extract = value_extract_expr(field);
        if field.optional {
            fields.push_str(&format!(
                "            {name}: {extract},\n",
                name = field.name,
                extract = extract,
            ));
        } else {
            fields.push_str(&format!(
                "            {name}: {extract}?,\n",
                name = field.name,
                extract = extract,
            ));
        }
    }
    Ok(format!(
        r#"
impl {name} {{
    /// Decode one Iris [`Row`] into `{name}` (flat scalar fields).
    pub fn from_row(row: &Row) -> Option<Self> {{
        Some(Self {{
{fields}        }})
    }}
}}
"#,
        name = table.name,
        fields = fields,
    ))
}

fn value_extract_expr(field: &FieldModel) -> String {
    let base = scalar_base(&field.rust_ty);
    match base {
        "bool" => format!("__iris_value_bool(row, \"{}\")", field.name),
        "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" => {
            format!(
                "__iris_value_i64(row, \"{}\").map(|v| v as {base})",
                field.name
            )
        }
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

fn emit_where_struct(table: &TableModel) -> Result<String> {
    let mut fields = String::new();
    for field in &table.fields {
        if field.reference_target.is_some() {
            continue;
        }
        fields.push_str(&format!(
            "    pub {}: {},\n",
            field.name,
            where_field_ty(field)
        ));
    }
    Ok(format!(
        r#"
/// Flat equality / bool filter for `{name}` (omit fields with `None`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct {name}Where {{
{fields}}}
"#,
        name = table.name,
        fields = fields,
    ))
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
            preds.push(format!("x.{fname} == \"{{}}\"", __iris_escape_vos_str(v)));
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

fn emit_db_delegate(table: &TableModel) -> Result<String> {
    let name = &table.name;
    let pk = primary_key(table)?;
    let synth = synthesize_fns(table);
    let to_write = row_write_fn(table, pk);
    Ok(format!(
        r#"
/// Table delegate for `{name}`.
pub struct {name}Delegate<'a> {{
    db: &'a Db<'a>,
}}

impl {name}Delegate<'_> {{
{synth}
{to_write}
    /// Find many rows matching a flat where.
    pub fn find_many(&self, where_: &{name}Where) -> MysqlResult<Vec<{name}>> {{
        let source = Self::synthesize_find_many(where_);
        let rows = self.db.query(&source)?;
        Ok(rows.iter().filter_map({name}::from_row).collect())
    }}

    /// Find at most one row (adds `.take(1)`).
    pub fn find_unique(&self, where_: &{name}Where) -> MysqlResult<Option<{name}>> {{
        let source = Self::synthesize_find_unique(where_);
        let rows = self.db.query(&source)?;
        Ok(rows.first().and_then({name}::from_row))
    }}

    /// Insert a row.
    pub fn insert(&self, row: &{name}) -> MysqlResult<()> {{
        self.db.source.insert(&Self::to_row_write(row))
    }}

    /// Update by primary key.
    pub fn update(&self, row: &{name}) -> MysqlResult<u64> {{
        self.db.source.update(&Self::to_row_write(row))
    }}

    /// Delete by primary key.
    pub fn delete(&self, key: &Value) -> MysqlResult<u64> {{
        self.db.source.delete("{name}", "{pk}", key)
    }}
}}
"#,
        name = name,
        pk = pk,
        synth = synth,
        to_write = to_write,
    ))
}

fn emit_txn_delegate(table: &TableModel) -> Result<String> {
    let name = &table.name;
    let pk = primary_key(table)?;
    Ok(format!(
        r#"
/// Table delegate for `{name}` while a [`Txn`] holds the connection.
pub struct {name}TxnDelegate<'a> {{
    source: &'a MysqlSource,
    planner: &'a Planner,
    conn: &'a mut iris_adapter_mysql::PooledConn,
}}

impl {name}TxnDelegate<'_> {{
    /// Find many rows matching a flat where.
    pub fn find_many(&mut self, where_: &{name}Where) -> MysqlResult<Vec<{name}>> {{
        let source = {name}Delegate::synthesize_find_many(where_);
        let plan = self
            .planner
            .plan_source(&source)
            .map_err(|e| iris_adapter_mysql::Error::Policy(e.to_string()))?;
        let rows = self.source.execute_plan_on(self.conn, &plan)?;
        Ok(rows.iter().filter_map({name}::from_row).collect())
    }}

    /// Find at most one row (adds `.take(1)`).
    pub fn find_unique(&mut self, where_: &{name}Where) -> MysqlResult<Option<{name}>> {{
        let source = {name}Delegate::synthesize_find_unique(where_);
        let plan = self
            .planner
            .plan_source(&source)
            .map_err(|e| iris_adapter_mysql::Error::Policy(e.to_string()))?;
        let rows = self.source.execute_plan_on(self.conn, &plan)?;
        Ok(rows.first().and_then({name}::from_row))
    }}

    /// Insert a row.
    pub fn insert(&mut self, row: &{name}) -> MysqlResult<()> {{
        self.source
            .insert_on(self.conn, &{name}Delegate::to_row_write(row))
    }}

    /// Update by primary key.
    pub fn update(&mut self, row: &{name}) -> MysqlResult<u64> {{
        self.source
            .update_on(self.conn, &{name}Delegate::to_row_write(row))
    }}

    /// Delete by primary key.
    pub fn delete(&mut self, key: &Value) -> MysqlResult<u64> {{
        self.source.delete_on(self.conn, "{name}", "{pk}", key)
    }}
}}
"#,
        name = name,
        pk = pk,
    ))
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
