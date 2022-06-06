//! Encode structured Iris operations into VOS source (generated client ABI).

use iris_wasm::bind;
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
    DeclaredVos {
        source: String,
    },
}

#[derive(Debug, Deserialize)]
pub struct WhereEq {
    pub field: String,
    pub value: JsonValue,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationIdentity {
    pub operation_id: String,
    pub contract_fingerprint: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationRequest {
    pub identity: OperationIdentity,
    pub operation: IrisOperation,
    pub parameters: Option<JsonValue>,
    pub deadline_ms: Option<u64>,
}

pub fn encode_operation_json(json: &str) -> Result<String, String> {
    let op: IrisOperation = serde_json::from_str(json).map_err(|err| err.to_string())?;
    encode_operation(&op)
}

pub fn encode_request_json(json: &str) -> Result<String, String> {
    let request: OperationRequest = serde_json::from_str(json).map_err(|err| err.to_string())?;
    let source = encode_operation(&request.operation)?;
    match request.parameters.as_ref() {
        Some(value) => {
            let parameters_json = serde_json::to_string(value).map_err(|err| err.to_string())?;
            bind::bind_parameters(&source, &parameters_json)
        }
        None => Ok(source),
    }
}

fn encode_operation(op: &IrisOperation) -> Result<String, String> {
    match op {
        IrisOperation::FindMany { entity, filter, take: _ } => {
            let pipeline = match filter {
                None => format!("{entity}.collect()"),
                Some(pred) => format!("{entity}.filter(x => x.{field}{cmp}).collect()", field = pred.field, cmp = cmp_suffix(&pred.value)?),
            };
            Ok(pipeline)
        }
        IrisOperation::FindUnique { entity, filter } => {
            let pipeline = format!("{entity}.filter(x => x.{field}{cmp}).collect()", field = filter.field, cmp = cmp_suffix(&filter.value)?);
            Ok(pipeline)
        }
        IrisOperation::DeclaredVos { source } => Ok(source.clone()),
    }
}

/// Bind optional request parameters into an operation before planning.
pub fn bind_operation_request(operation_json: &str, parameters_json: Option<&str>) -> Result<String, String> {
    let source = encode_operation_json(operation_json)?;
    match parameters_json {
        Some(json) => bind::bind_parameters(&source, json),
        None => Ok(source),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_vos_binds_parameters() {
        let request = r#"{
            "identity": { "operationId": "User.findMany", "contractFingerprint": "fp" },
            "operation": { "kind": "declared-vos", "source": "User.filter(x => x.active == $where_active).collect()" },
            "parameters": { "where_active": true }
        }"#;
        let bound = encode_request_json(request).expect("request");
        assert_eq!(bound, "User.filter(x => x.active == true).collect()");
    }
}
