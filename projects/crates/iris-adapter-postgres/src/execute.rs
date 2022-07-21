//! Physical plan execution and row writes (private PostgreSQL SQL).

use iris_ir::{CmpOp, LiteralKind, PhysicalOp, PhysicalPlan, Pred, ProjectField, WriteField};
use iris_types::{Row, RowWrite, Value};
use postgres::{
    Client, GenericClient,
    types::{IsNull, ToSql, Type, to_sql_checked},
};

use crate::Result;

#[derive(Debug, Clone)]
enum OwnedSql {
    Null,
    Bool(bool),
    Int(i64),
    Text(String),
}

impl ToSql for OwnedSql {
    fn to_sql(&self, ty: &Type, out: &mut bytes::BytesMut) -> std::result::Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
        match self {
            Self::Null => Ok(IsNull::Yes),
            Self::Bool(v) => v.to_sql(ty, out),
            Self::Int(v) => v.to_sql(ty, out),
            Self::Text(v) => v.to_sql(ty, out),
        }
    }

    fn accepts(ty: &Type) -> bool {
        <bool as ToSql>::accepts(ty)
            || <i64 as ToSql>::accepts(ty)
            || <String as ToSql>::accepts(ty)
            || matches!(*ty, Type::UNKNOWN | Type::TEXT | Type::VARCHAR | Type::BPCHAR)
    }

    to_sql_checked!();
}

fn to_owned(value: &Value) -> OwnedSql {
    match value {
        Value::Null => OwnedSql::Null,
        Value::Bool(b) => OwnedSql::Bool(*b),
        Value::Int(i) => OwnedSql::Int(*i),
        Value::Str(s) => OwnedSql::Text(s.clone()),
        Value::Object(_) => OwnedSql::Null,
    }
}

pub(crate) fn execute_plan(client: &mut Client, plan: &PhysicalPlan) -> Result<Vec<Row>> {
    if plan
        .nodes
        .iter()
        .any(|node| matches!(node.op, PhysicalOp::Insert { .. } | PhysicalOp::Patch { .. } | PhysicalOp::Delete { .. }))
    {
        return execute_write_plan(client, plan);
    }
    execute_read_plan(client, plan)
}

fn execute_read_plan(client: &mut Client, plan: &PhysicalPlan) -> Result<Vec<Row>> {
    let mut table: Option<String> = None;
    let mut where_sql: Option<String> = None;
    let mut where_params: Vec<OwnedSql> = Vec::new();
    let mut order: Option<String> = None;
    let mut limit: Option<u64> = None;
    let mut offset: Option<u64> = None;
    let mut projection: Option<Vec<String>> = None;
    let mut next_param = 1usize;

    for node in &plan.nodes {
        match &node.op {
            PhysicalOp::Scan { table: t } => table = Some(t.clone()),
            PhysicalOp::Filter { predicate } => {
                let (sql, params, next) = pred_to_sql(predicate, next_param)?;
                where_sql = Some(sql);
                where_params = params;
                next_param = next;
            }
            PhysicalOp::Project { fields } => {
                projection = Some(
                    fields
                        .iter()
                        .map(|f| {
                            let src = f.from.as_deref().unwrap_or(f.name.as_str());
                            if src == f.name { format!("\"{src}\"") } else { format!("\"{src}\" AS \"{}\"", f.name) }
                        })
                        .collect(),
                );
            }
            PhysicalOp::Sort { keys } => {
                let parts: Vec<String> = keys.iter().map(|k| format!("\"{}\" {}", k.field, if k.ascending { "ASC" } else { "DESC" })).collect();
                order = Some(parts.join(", "));
            }
            PhysicalOp::Skip { count } => offset = Some(*count),
            PhysicalOp::Take { count } => limit = Some(*count),
            PhysicalOp::Collect => {}
            PhysicalOp::Insert { .. } | PhysicalOp::Patch { .. } | PhysicalOp::Delete { .. } => {
                return Err(crate::Error::Policy("write op in read plan".into()));
            }
        }
    }

    let table = table.ok_or_else(|| crate::Error::Policy("plan missing Scan".into()))?;
    let select = projection.map(|p| p.join(", ")).unwrap_or_else(|| "*".into());
    let mut sql = format!("SELECT {select} FROM \"{table}\"");
    if let Some(w) = &where_sql {
        sql.push_str(" WHERE ");
        sql.push_str(w);
    }
    if let Some(o) = &order {
        sql.push_str(" ORDER BY ");
        sql.push_str(o);
    }
    if let Some(l) = limit {
        sql.push_str(&format!(" LIMIT {l}"));
    }
    if let Some(o) = offset {
        sql.push_str(&format!(" OFFSET {o}"));
    }

    let param_refs: Vec<&(dyn ToSql + Sync)> = where_params.iter().map(|p| p as &(dyn ToSql + Sync)).collect();
    let rows = client.query(&sql, &param_refs[..])?;
    let mut out = Vec::new();
    for row in rows {
        let mut r = Row::new();
        for (idx, col) in row.columns().iter().enumerate() {
            r.insert(col.name().to_string(), from_pg_row(&row, idx));
        }
        out.push(r);
    }
    let _ = next_param;
    Ok(out)
}

fn execute_write_plan(client: &mut Client, plan: &PhysicalPlan) -> Result<Vec<Row>> {
    let mut insert: Option<(String, Vec<WriteField>)> = None;
    let mut patch: Option<(String, Option<Pred>, Vec<WriteField>)> = None;
    let mut delete: Option<(String, Option<Pred>)> = None;
    let mut projection: Option<Vec<ProjectField>> = None;

    for node in &plan.nodes {
        match &node.op {
            PhysicalOp::Insert { table, fields } => insert = Some((table.clone(), fields.clone())),
            PhysicalOp::Patch { table, filter, fields } => patch = Some((table.clone(), filter.clone(), fields.clone())),
            PhysicalOp::Delete { table, filter } => delete = Some((table.clone(), filter.clone())),
            PhysicalOp::Project { fields } => projection = Some(fields.clone()),
            PhysicalOp::Collect => {}
            other => return Err(crate::Error::Policy(format!("unexpected op in write plan: {other:?}"))),
        }
    }

    if let Some((table, filter)) = delete {
        let (where_sql, params, _) = filter_to_sql(filter.as_ref(), 1)?;
        let sql = format!("DELETE FROM \"{table}\" WHERE {where_sql}");
        let param_refs: Vec<&(dyn ToSql + Sync)> = params.iter().map(|p| p as &(dyn ToSql + Sync)).collect();
        client.execute(&sql, &param_refs[..])?;
        return Ok(Vec::new());
    }

    if let Some((table, fields)) = insert {
        let returning = returning_sql(projection.as_deref());
        let cols: Vec<String> = fields.iter().map(|f| format!("\"{}\"", f.name)).collect();
        let placeholders: Vec<String> = (1..=cols.len()).map(|i| format!("${i}")).collect();
        let sql = format!(
            "INSERT INTO \"{table}\" ({}) VALUES ({}) RETURNING {returning}",
            cols.join(", "),
            placeholders.join(", ")
        );
        let params: Vec<OwnedSql> = fields.iter().map(write_field_to_owned).collect();
        let param_refs: Vec<&(dyn ToSql + Sync)> = params.iter().map(|p| p as &(dyn ToSql + Sync)).collect();
        let mut rows = query_rows(client, &sql, &param_refs)?;
        if let Some(fields) = projection.as_ref() {
            rows = apply_projection(rows, fields);
        }
        return Ok(rows);
    }

    if let Some((table, filter, fields)) = patch {
        let sets: Vec<String> = fields.iter().enumerate().map(|(i, f)| format!("\"{}\" = ${}", f.name, i + 1)).collect();
        let (where_sql, mut where_params, _) = filter_to_sql(filter.as_ref(), fields.len() + 1)?;
        let returning = returning_sql(projection.as_deref());
        let sql = format!(
            "UPDATE \"{table}\" SET {} WHERE {where_sql} RETURNING {returning}",
            sets.join(", ")
        );
        let mut params: Vec<OwnedSql> = fields.iter().map(write_field_to_owned).collect();
        params.extend(where_params.drain(..));
        let param_refs: Vec<&(dyn ToSql + Sync)> = params.iter().map(|p| p as &(dyn ToSql + Sync)).collect();
        let mut rows = query_rows(client, &sql, &param_refs)?;
        if let Some(fields) = projection.as_ref() {
            rows = apply_projection(rows, fields);
        }
        return Ok(rows);
    }

    Ok(Vec::new())
}

fn query_rows(client: &mut Client, sql: &str, param_refs: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>> {
    let rows = client.query(sql, param_refs)?;
    let mut out = Vec::new();
    for row in rows {
        let mut r = Row::new();
        for (idx, col) in row.columns().iter().enumerate() {
            r.insert(col.name().to_string(), from_pg_row(&row, idx));
        }
        out.push(r);
    }
    Ok(out)
}

fn project_fields_sql(fields: &[ProjectField]) -> Vec<String> {
    fields
        .iter()
        .map(|f| {
            let src = f.from.as_deref().unwrap_or(f.name.as_str());
            if src == f.name { format!("\"{src}\"") } else { format!("\"{src}\" AS \"{}\"", f.name) }
        })
        .collect()
}

fn returning_sql(projection: Option<&[ProjectField]>) -> String {
    projection
        .map(|fields| project_fields_sql(fields).join(", "))
        .unwrap_or_else(|| "*".into())
}

fn apply_projection(rows: Vec<Row>, fields: &[ProjectField]) -> Vec<Row> {
    rows.into_iter()
        .map(|row| {
            let mut out = Row::new();
            for field in fields {
                let src = field.from.as_deref().unwrap_or(field.name.as_str());
                if let Some(value) = row.get(src) {
                    out.insert(field.name.clone(), value.clone());
                }
            }
            out
        })
        .collect()
}

fn filter_to_sql(filter: Option<&Pred>, start: usize) -> Result<(String, Vec<OwnedSql>, usize)> {
    match filter {
        Some(pred) => pred_to_sql(pred, start),
        None => Ok(("TRUE".into(), Vec::new(), start)),
    }
}

fn write_field_to_owned(field: &WriteField) -> OwnedSql {
    literal_to_owned(&field.literal, field.kind)
}

pub(crate) fn insert_row(client: &mut impl GenericClient, write: &RowWrite) -> Result<()> {
    let cols: Vec<String> = write.fields.keys().map(|k| format!("\"{k}\"")).collect();
    let placeholders: Vec<String> = (1..=cols.len()).map(|i| format!("${i}")).collect();
    let sql = format!("INSERT INTO \"{}\" ({}) VALUES ({})", write.table, cols.join(", "), placeholders.join(", "));
    let params: Vec<OwnedSql> = write.fields.values().map(to_owned).collect();
    let param_refs: Vec<&(dyn ToSql + Sync)> = params.iter().map(|p| p as &(dyn ToSql + Sync)).collect();
    client.execute(&sql, &param_refs[..])?;
    Ok(())
}

pub(crate) fn update_row(client: &mut impl GenericClient, write: &RowWrite) -> Result<u64> {
    let key = write.fields.get(&write.primary_key).ok_or_else(|| crate::Error::Policy("update missing primary key value".into()))?;
    let set_fields: Vec<&String> = write.fields.keys().filter(|k| *k != &write.primary_key).collect();
    if set_fields.is_empty() {
        return Ok(0);
    }
    let sets: Vec<String> = set_fields.iter().enumerate().map(|(i, k)| format!("\"{k}\" = ${}", i + 1)).collect();
    let key_idx = set_fields.len() + 1;
    let sql = format!("UPDATE \"{}\" SET {} WHERE \"{}\" = ${key_idx}", write.table, sets.join(", "), write.primary_key);
    let mut params: Vec<OwnedSql> = set_fields.iter().map(|k| to_owned(write.fields.get(*k).expect("field"))).collect();
    params.push(to_owned(key));
    let param_refs: Vec<&(dyn ToSql + Sync)> = params.iter().map(|p| p as &(dyn ToSql + Sync)).collect();
    Ok(client.execute(&sql, &param_refs[..])?)
}

pub(crate) fn delete_row(client: &mut impl GenericClient, table: &str, primary_key: &str, key: &Value) -> Result<u64> {
    let sql = format!("DELETE FROM \"{table}\" WHERE \"{primary_key}\" = $1");
    let param = to_owned(key);
    Ok(client.execute(&sql, &[&param])?)
}

fn pred_to_sql(pred: &Pred, start: usize) -> Result<(String, Vec<OwnedSql>, usize)> {
    match pred {
        Pred::FieldBool { field, value } => Ok((format!("\"{field}\" = ${start}"), vec![OwnedSql::Bool(*value)], start + 1)),
        Pred::FieldCmp { field, op, literal, kind } => {
            let op_sql = match op {
                CmpOp::Eq => "=",
                CmpOp::Ne => "!=",
                CmpOp::Lt => "<",
                CmpOp::Le => "<=",
                CmpOp::Gt => ">",
                CmpOp::Ge => ">=",
            };
            Ok((format!("\"{field}\" {op_sql} ${start}"), vec![literal_to_owned(literal, *kind)], start + 1))
        }
        Pred::And(a, b) => {
            let (sa, mut pa, n1) = pred_to_sql(a, start)?;
            let (sb, pb, n2) = pred_to_sql(b, n1)?;
            pa.extend(pb);
            Ok((format!("({sa}) AND ({sb})"), pa, n2))
        }
        Pred::Or(a, b) => {
            let (sa, mut pa, n1) = pred_to_sql(a, start)?;
            let (sb, pb, n2) = pred_to_sql(b, n1)?;
            pa.extend(pb);
            Ok((format!("({sa}) OR ({sb})"), pa, n2))
        }
    }
}

fn literal_to_owned(text: &str, kind: LiteralKind) -> OwnedSql {
    match kind {
        LiteralKind::Null => OwnedSql::Null,
        LiteralKind::Bool => OwnedSql::Bool(text == "true"),
        LiteralKind::Int => OwnedSql::Int(text.parse().unwrap_or(0)),
        LiteralKind::Str => OwnedSql::Text(text.to_string()),
    }
}

fn from_pg_row(row: &postgres::Row, idx: usize) -> Value {
    if let Ok(v) = row.try_get::<_, bool>(idx) {
        return Value::Bool(v);
    }
    if let Ok(v) = row.try_get::<_, i64>(idx) {
        return Value::Int(v);
    }
    if let Ok(v) = row.try_get::<_, i32>(idx) {
        return Value::Int(i64::from(v));
    }
    if let Ok(v) = row.try_get::<_, String>(idx) {
        return Value::Str(v);
    }
    if let Ok(Some(v)) = row.try_get::<_, Option<String>>(idx) {
        return Value::Str(v);
    }
    Value::Null
}
