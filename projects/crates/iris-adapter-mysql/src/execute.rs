//! Physical plan execution and row writes (private MySQL SQL).

use std::collections::HashSet;

use iris_ir::{CmpOp, LiteralKind, PhysicalOp, PhysicalPlan, Pred};
use iris_types::{Row, RowWrite, Value};
use mysql::PooledConn;
use mysql::prelude::*;
use mysql::{Params, Value as MysqlValue};

use crate::Result;
use crate::uuid_util::{try_parse_uuid_bytes, uuid_bytes_to_str};

pub(crate) fn execute_plan(
    conn: &mut PooledConn,
    plan: &PhysicalPlan,
    uuid_fields: &HashSet<(String, String)>,
) -> Result<Vec<Row>> {
    let mut table: Option<String> = None;
    let mut where_sql: Option<String> = None;
    let mut where_params: Vec<MysqlValue> = Vec::new();
    let mut order: Option<String> = None;
    let mut limit: Option<u64> = None;
    let mut offset: Option<u64> = None;
    let mut projection: Option<Vec<String>> = None;

    for node in &plan.nodes {
        match &node.op {
            PhysicalOp::Scan { table: t } => table = Some(t.clone()),
            PhysicalOp::Filter { predicate } => {
                let table_ref = table.as_deref().unwrap_or("");
                let (sql, params) = pred_to_sql(predicate, table_ref, uuid_fields)?;
                where_sql = Some(sql);
                where_params = params;
            }
            PhysicalOp::Project { fields } => {
                projection = Some(
                    fields
                        .iter()
                        .map(|f| {
                            let src = f.from.as_deref().unwrap_or(f.name.as_str());
                            if src == f.name {
                                format!("`{src}`")
                            } else {
                                format!("`{src}` AS `{}`", f.name)
                            }
                        })
                        .collect(),
                );
            }
            PhysicalOp::Sort { keys } => {
                let parts: Vec<String> = keys
                    .iter()
                    .map(|k| format!("`{}` {}", k.field, if k.ascending { "ASC" } else { "DESC" }))
                    .collect();
                order = Some(parts.join(", "));
            }
            PhysicalOp::Skip { count } => offset = Some(*count),
            PhysicalOp::Take { count } => limit = Some(*count),
            PhysicalOp::Collect => {}
        }
    }

    let table = table.ok_or_else(|| crate::Error::Policy("plan missing Scan".into()))?;
    let select = projection
        .map(|p| p.join(", "))
        .unwrap_or_else(|| "*".into());
    let mut sql = format!("SELECT {select} FROM `{table}`");
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

    let result = conn.exec_iter(sql, Params::Positional(where_params))?;
    let mut out = Vec::new();
    for row in result {
        let row = row?;
        let columns = row.columns_ref();
        let mut r = Row::new();
        for (idx, col) in columns.iter().enumerate() {
            let raw: MysqlValue = row.get(idx).unwrap_or(MysqlValue::NULL);
            let col_name = col.name_str();
            let field = col_name.as_ref();
            r.insert(
                field.to_string(),
                from_mysql(raw, &table, field, uuid_fields),
            );
        }
        out.push(r);
    }
    Ok(out)
}

pub(crate) fn insert_row(
    conn: &mut PooledConn,
    write: &RowWrite,
    uuid_fields: &HashSet<(String, String)>,
) -> Result<()> {
    let cols: Vec<String> = write.fields.keys().map(|k| format!("`{k}`")).collect();
    let placeholders: Vec<&str> = cols.iter().map(|_| "?").collect();
    let sql = format!(
        "INSERT INTO `{}` ({}) VALUES ({})",
        write.table,
        cols.join(", "),
        placeholders.join(", ")
    );
    let params: Vec<MysqlValue> = write
        .fields
        .iter()
        .map(|(k, v)| to_mysql(v, &write.table, k, uuid_fields))
        .collect();
    conn.exec_drop(sql, Params::Positional(params))?;
    Ok(())
}

pub(crate) fn update_row(
    conn: &mut PooledConn,
    write: &RowWrite,
    uuid_fields: &HashSet<(String, String)>,
) -> Result<u64> {
    let key = write
        .fields
        .get(&write.primary_key)
        .ok_or_else(|| crate::Error::Policy("update missing primary key value".into()))?;
    let sets: Vec<String> = write
        .fields
        .keys()
        .filter(|k| *k != &write.primary_key)
        .map(|k| format!("`{k}` = ?"))
        .collect();
    if sets.is_empty() {
        return Ok(0);
    }
    let sql = format!(
        "UPDATE `{}` SET {} WHERE `{}` = ?",
        write.table,
        sets.join(", "),
        write.primary_key
    );
    let mut params: Vec<MysqlValue> = write
        .fields
        .iter()
        .filter(|(k, _)| *k != &write.primary_key)
        .map(|(k, v)| to_mysql(v, &write.table, k, uuid_fields))
        .collect();
    params.push(to_mysql(key, &write.table, &write.primary_key, uuid_fields));
    conn.exec_drop(sql, Params::Positional(params))?;
    Ok(conn.affected_rows())
}

pub(crate) fn delete_row(
    conn: &mut PooledConn,
    table: &str,
    primary_key: &str,
    key: &Value,
    uuid_fields: &HashSet<(String, String)>,
) -> Result<u64> {
    let sql = format!("DELETE FROM `{table}` WHERE `{primary_key}` = ?");
    conn.exec_drop(
        sql,
        Params::Positional(vec![to_mysql(key, table, primary_key, uuid_fields)]),
    )?;
    Ok(conn.affected_rows())
}

fn pred_to_sql(
    pred: &Pred,
    table: &str,
    uuid_fields: &HashSet<(String, String)>,
) -> Result<(String, Vec<MysqlValue>)> {
    match pred {
        Pred::FieldBool { field, value } => Ok((
            format!("`{field}` = ?"),
            vec![MysqlValue::Int(i64::from(*value))],
        )),
        Pred::FieldCmp {
            field,
            op,
            literal,
            kind,
        } => {
            let op_sql = match op {
                CmpOp::Eq => "=",
                CmpOp::Ne => "!=",
                CmpOp::Lt => "<",
                CmpOp::Le => "<=",
                CmpOp::Gt => ">",
                CmpOp::Ge => ">=",
            };
            Ok((
                format!("`{field}` {op_sql} ?"),
                vec![literal_to_mysql(literal, *kind, table, field, uuid_fields)],
            ))
        }
        Pred::And(a, b) => {
            let (sa, mut pa) = pred_to_sql(a, table, uuid_fields)?;
            let (sb, pb) = pred_to_sql(b, table, uuid_fields)?;
            pa.extend(pb);
            Ok((format!("({sa}) AND ({sb})"), pa))
        }
        Pred::Or(a, b) => {
            let (sa, mut pa) = pred_to_sql(a, table, uuid_fields)?;
            let (sb, pb) = pred_to_sql(b, table, uuid_fields)?;
            pa.extend(pb);
            Ok((format!("({sa}) OR ({sb})"), pa))
        }
    }
}

fn literal_to_mysql(
    text: &str,
    kind: LiteralKind,
    table: &str,
    field: &str,
    uuid_fields: &HashSet<(String, String)>,
) -> MysqlValue {
    match kind {
        LiteralKind::Null => MysqlValue::NULL,
        LiteralKind::Bool => MysqlValue::Int(i64::from(text == "true")),
        LiteralKind::Int => MysqlValue::Int(text.parse().unwrap_or(0)),
        LiteralKind::Str => encode_str(text, table, field, uuid_fields),
    }
}

fn to_mysql(
    value: &Value,
    table: &str,
    field: &str,
    uuid_fields: &HashSet<(String, String)>,
) -> MysqlValue {
    match value {
        Value::Null => MysqlValue::NULL,
        Value::Bool(b) => MysqlValue::Int(i64::from(*b)),
        Value::Int(i) => MysqlValue::Int(*i),
        Value::Str(s) => encode_str(s, table, field, uuid_fields),
    }
}

fn encode_str(
    text: &str,
    table: &str,
    field: &str,
    uuid_fields: &HashSet<(String, String)>,
) -> MysqlValue {
    if uuid_fields.contains(&(table.to_string(), field.to_string()))
        && let Some(bytes) = try_parse_uuid_bytes(text)
    {
        return MysqlValue::Bytes(bytes);
    }
    MysqlValue::Bytes(text.as_bytes().to_vec())
}

fn from_mysql(
    value: MysqlValue,
    table: &str,
    field: &str,
    uuid_fields: &HashSet<(String, String)>,
) -> Value {
    match value {
        MysqlValue::NULL => Value::Null,
        MysqlValue::Int(i) => Value::Int(i),
        MysqlValue::UInt(i) => Value::Int(i as i64),
        MysqlValue::Bytes(b) => {
            if uuid_fields.contains(&(table.to_string(), field.to_string()))
                && let Some(text) = uuid_bytes_to_str(&b)
            {
                return Value::Str(text);
            }
            Value::Str(String::from_utf8_lossy(&b).into_owned())
        }
        MysqlValue::Float(f) => Value::Str(f.to_string()),
        MysqlValue::Double(f) => Value::Str(f.to_string()),
        other => Value::Str(format!("{other:?}")),
    }
}
