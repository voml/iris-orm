//! End-to-end evidence for the `i64` and `bytes` carriers.
//!
//! The chain under test is the one every generated client uses: a bound VOS
//! parameter record → `bind_parameters` → row-producing VOS → host row JSON.
//! No provider crate is involved, so this covers parameter binding, the
//! reference interpreter's row values and the JSON wire. A writable real
//! datastore binding is not available in this tree (the SQLite adapter is
//! excluded and the reference store is read-only), so nothing here claims a
//! provider round trip.

use std::collections::BTreeMap;

use iris_types::{ReferenceStore, Value};
use iris_wasm::{bind_parameters, wire::rows_to_json};
use serde_json::Value as JsonValue;

const INSERT: &str = "Metric::insert({ metric_id: $data_metric_id, seq: $data_seq, payload: $data_payload })";

/// `i64::MAX` with bytes `[1, 2, 3]` as base64 (`AQID`).
const PARAMS: &str = r#"{"data_metric_id":"m1","data_seq":9223372036854775807,"data_payload":"AQID"}"#;

fn metric_row() -> BTreeMap<String, Value> {
    let mut row = BTreeMap::new();
    row.insert("metric_id".to_string(), Value::Str("m1".into()));
    row.insert("seq".to_string(), Value::Int(i64::MAX));
    row.insert("payload".to_string(), Value::Str("AQID".into()));
    row
}

#[test]
fn binds_integer_parameter_with_every_bit() {
    let bound = bind_parameters(INSERT, PARAMS).expect("bind");
    assert!(bound.contains("seq: 9223372036854775807"), "{bound}");
    assert!(bound.contains("payload: \"AQID\""), "{bound}");
}

#[test]
fn rejects_an_integer_parameter_outside_i64() {
    let err = bind_parameters("Metric.filter(x => x.seq == $p_seq)", r#"{"p_seq":18446744073709551615}"#).expect_err("u64 max");
    assert!(err.contains("i64"), "{err}");
}

#[test]
fn bound_i64_and_bytes_reach_the_row_values() {
    let store = ReferenceStore::new();
    let insert = bind_parameters(INSERT, PARAMS).expect("bind insert");
    let rows = store.interpret_source(&insert).expect("insert");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("seq"), Some(&Value::Int(i64::MAX)));
    assert_eq!(rows[0].get("payload"), Some(&Value::Str("AQID".into())));
}

#[test]
fn exact_i64_filter_matches_and_survives_the_wire() {
    let mut store = ReferenceStore::new();
    store.seed("Metric", vec![metric_row()]);

    let query = bind_parameters("Metric.filter(x => x.seq == $p_seq).collect()", r#"{"p_seq":9223372036854775807}"#).expect("bind query");
    let rows = store.interpret_source(&query).expect("query");
    assert_eq!(rows.len(), 1);

    let json = rows_to_json(rows);
    assert_eq!(json[0]["seq"], JsonValue::String("9223372036854775807".into()));
    assert_eq!(json[0]["payload"], JsonValue::String("AQID".into()));
}

#[test]
fn wire_keeps_small_integers_as_numbers() {
    let mut row = BTreeMap::new();
    row.insert("seq".to_string(), Value::Int(42));
    row.insert("negative".to_string(), Value::Int(-9_007_199_254_740_991));

    let json = rows_to_json(vec![row]);
    assert_eq!(json[0]["seq"], JsonValue::from(42));
    assert_eq!(json[0]["negative"], JsonValue::from(-9_007_199_254_740_991i64));
}
