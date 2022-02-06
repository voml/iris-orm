//! Encode structured Iris operations into VOS source (generated client ABI).

use serde::Deserialize;
use serde_json::Value as JsonValue;

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum IrisOperation {
    FindMany {
        entity: String,
        #[serde(rename = "where")]
        filter: Option<WhereEq>,
        #[allow(dead_code)]
        take: Option<u32>,
    },
    FindUnique {
        entity: String,
        #[serde(rename = "where")]
        filter: WhereEq,
    },
}

#[derive(Debug, Deserialize)]
pub struct WhereEq {
    pub field: String,
    pub value: JsonValue,
}

pub fn encode_operation_json(json: &str) -> Result<String, String> {
    let op: IrisOperation = serde_json::from_str(json).map_err(|err| err.to_string())?;
    encode_operation(&op)
}

fn encode_operation(op: &IrisOperation) -> Result<String, String> {
    match op {
        IrisOperation::FindMany { entity, filter, take: _ } => {
            let pipeline = match filter {
                None => format!("{entity}.collect()"),
                Some(pred) => format!(
                    "{entity}.filter(x => x.{field}{cmp}).collect()",
                    field = pred.field,
                    cmp = cmp_suffix(&pred.value)?
                ),
            };
            Ok(pipeline)
        }
        IrisOperation::FindUnique { entity, filter } => {
            let pipeline = format!(
                "{entity}.filter(x => x.{field}{cmp}).collect()",
                field = filter.field,
                cmp = cmp_suffix(&filter.value)?
            );
            Ok(pipeline)
        }
    }
}

fn cmp_suffix(value: &JsonValue) -> Result<String, String> {
    match value {
        JsonValue::Bool(true) => Ok(String::new()),
        JsonValue::Bool(false) => Ok(".eq(false)".into()),
        JsonValue::Number(n) => Ok(format!(".eq({n})")),
        JsonValue::String(s) => Ok(format!(".eq(\"{s}\")")),
        JsonValue::Null => Ok(".eq(null)".into()),
        _ => Err("unsupported predicate value in operation ABI".into()),
    }
}
