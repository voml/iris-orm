//! Stateful in-memory reference session for browser WASM hosts.

use iris::{CapabilitySet, Iris, ReferenceStore, Row};
use serde_json::{Map, Value as JsonValue, json};
use wasm_bindgen::prelude::*;

fn rows_to_json(rows: Vec<Row>) -> Vec<JsonValue> {
    rows.into_iter()
        .map(|row| {
            let mut obj = Map::new();
            for (key, value) in row {
                obj.insert(key, value_to_json(&value));
            }
            JsonValue::Object(obj)
        })
        .collect()
}

fn value_to_json(value: &iris::Value) -> JsonValue {
    match value {
        iris::Value::Null => JsonValue::Null,
        iris::Value::Bool(b) => json!(b),
        iris::Value::Int(i) => json!(i),
        iris::Value::Str(s) => json!(s),
    }
}

fn execute_result_json(source: &str, iris: &Iris) -> String {
    match iris.session().execute_vos(source) {
        Ok(rows) => json!({ "ok": true, "rows": rows_to_json(rows), "error": JsonValue::Null }).to_string(),
        Err(err) => json!({ "ok": false, "rows": [], "error": err.to_string() }).to_string(),
    }
}

/// Execute VOS against a fresh in-memory reference adapter (stateless helper).
pub fn execute_vos_memory(source: &str) -> String {
    let iris = Iris::new(CapabilitySet::reference_full(), ReferenceStore::new());
    execute_result_json(source, &iris)
}

/// Stateful in-memory reference session.
#[wasm_bindgen]
pub struct MemorySession {
    iris: Iris,
    closed: bool,
}

#[wasm_bindgen]
impl MemorySession {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            iris: Iris::new(CapabilitySet::reference_full(), ReferenceStore::new()),
            closed: false,
        }
    }

    #[wasm_bindgen(js_name = executeVos)]
    pub fn execute_vos(&self, source: &str) -> Result<String, JsValue> {
        if self.closed {
            return Err(JsValue::from_str("session closed"));
        }
        Ok(execute_result_json(source, &self.iris))
    }

    #[wasm_bindgen]
    pub fn close(&mut self) {
        self.closed = true;
    }
}
