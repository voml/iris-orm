//! Physical plan execution and row writes (private SQL only).

use iris_ir::{CmpOp, LiteralKind, PhysicalOp, PhysicalPlan, Pred};
use iris_types::{Row, RowWrite, Value};
use rusqlite::{Connection, params_from_iter};

use crate::Result;
use crate::types::{from_sql_value, to_sql_value};

pub(crate) fn execute_plan(conn: &Connection, plan: &PhysicalPlan) -> Result<Vec<Row>> {
    let mut table: Option<String> = None;
    let mut where_sql: Option<String> = None;
    let mut where_params: Vec<rusqlite::types::Value> = Vec::new();
    let mut order: Option<String> = None;
    let mut limit: Option<u64> = None;
    let mut offset: Option<u64> = None;
    let mut projection: Option<Vec<String>> = None;

    for node in &plan.nodes {
        match &node.op {
            PhysicalOp::Scan { table: t } => {
                table = Some(t.clone());
            }
            PhysicalOp::Filter { predicate } => {
                let (sql, params) = pred_to_sql(predicate)?;
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
                                format!("\"{src}\"")
                            } else {
                                format!("\"{src}\" AS \"{}\"", f.name)
                            }
                        })
                        .collect(),
                );
            }
            PhysicalOp::Sort { keys } => {
                let parts: Vec<String> = keys
                    .iter()
                    .map(|k| {
                        format!(
                            "\"{}\" {}",
                            k.field,
                            if k.ascending { "ASC" } else { "DESC" }
                        )
                    })
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

    let mut stmt = conn.prepare(&sql)?;
    let column_names: Vec<String> = stmt
        .column_names()
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let rows = stmt
        .query_map(params_from_iter(where_params), |row| {
            let mut out = Row::new();
            for (idx, name) in column_names.iter().enumerate() {
                let raw: rusqlite::types::Value = row.get(idx)?;
                out.insert(name.clone(), from_sql_value(raw));
            }
            Ok(out)
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub(crate) fn insert_row(conn: &Connection, write: &RowWrite) -> Result<()> {
    let cols: Vec<String> = write.fields.keys().map(|k| format!("\"{k}\"")).collect();
    let placeholders: Vec<&str> = cols.iter().map(|_| "?").collect();
    let sql = format!(
        "INSERT INTO \"{}\" ({}) VALUES ({})",
        write.table,
        cols.join(", "),
        placeholders.join(", ")
    );
    let params: Vec<rusqlite::types::Value> = write.fields.values().map(to_sql_value).collect();
    conn.execute(&sql, params_from_iter(params))?;
    Ok(())
}

pub(crate) fn update_row(conn: &Connection, write: &RowWrite) -> Result<usize> {
    let key = write
        .fields
        .get(&write.primary_key)
        .ok_or_else(|| crate::Error::Policy("update missing primary key value".into()))?;
    let sets: Vec<String> = write
        .fields
        .keys()
        .filter(|k| *k != &write.primary_key)
        .map(|k| format!("\"{k}\" = ?"))
        .collect();
    if sets.is_empty() {
        return Ok(0);
    }
    let sql = format!(
        "UPDATE \"{}\" SET {} WHERE \"{}\" = ?",
        write.table,
        sets.join(", "),
        write.primary_key
    );
    let mut params: Vec<rusqlite::types::Value> = write
        .fields
        .iter()
        .filter(|(k, _)| *k != &write.primary_key)
        .map(|(_, v)| to_sql_value(v))
        .collect();
    params.push(to_sql_value(key));
    let n = conn.execute(&sql, params_from_iter(params))?;
    Ok(n)
}

pub(crate) fn delete_row(
    conn: &Connection,
    table: &str,
    primary_key: &str,
    key: &Value,
) -> Result<usize> {
    let sql = format!("DELETE FROM \"{table}\" WHERE \"{primary_key}\" = ?");
    let n = conn.execute(&sql, params_from_iter([to_sql_value(key)]))?;
    Ok(n)
}

fn pred_to_sql(pred: &Pred) -> Result<(String, Vec<rusqlite::types::Value>)> {
    match pred {
        Pred::FieldBool { field, value } => Ok((
            format!("\"{field}\" = ?"),
            vec![rusqlite::types::Value::Integer(i64::from(*value))],
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
                format!("\"{field}\" {op_sql} ?"),
                vec![literal_to_sql(literal, *kind)],
            ))
        }
        Pred::And(a, b) => {
            let (sa, mut pa) = pred_to_sql(a)?;
            let (sb, pb) = pred_to_sql(b)?;
            pa.extend(pb);
            Ok((format!("({sa}) AND ({sb})"), pa))
        }
        Pred::Or(a, b) => {
            let (sa, mut pa) = pred_to_sql(a)?;
            let (sb, pb) = pred_to_sql(b)?;
            pa.extend(pb);
            Ok((format!("({sa}) OR ({sb})"), pa))
        }
    }
}

fn literal_to_sql(text: &str, kind: LiteralKind) -> rusqlite::types::Value {
    match kind {
        LiteralKind::Null => rusqlite::types::Value::Null,
        LiteralKind::Bool => rusqlite::types::Value::Integer(i64::from(text == "true")),
        LiteralKind::Int => rusqlite::types::Value::Integer(text.parse().unwrap_or(0)),
        LiteralKind::Str => rusqlite::types::Value::Text(text.to_string()),
    }
}
