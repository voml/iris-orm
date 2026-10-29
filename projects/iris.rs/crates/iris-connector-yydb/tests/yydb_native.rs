//! Phase 2: native YYDB connector — VOS executor live on the Connection facade.

use std::{
    collections::BTreeMap,
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use iris_connector_yydb::{BACKEND_ID, YydbSource, value_from_yydb, value_params_to_yydb};

const USER_SCHEMA: &str = r#"
table User {
    @@user_id: utf8,
    @user_name: utf8,
    active: bool,
}
"#;

fn temp_layout_path(label: &str, extension: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("iris-yydb-{label}-{nanos}"));
    fs::create_dir_all(&dir).expect("temp dir");
    dir.join(format!("db.{extension}"))
}

fn temp_db_path(label: &str) -> std::path::PathBuf {
    temp_layout_path(label, "yydb")
}

// ---------------------------------------------------------------------------
// Readiness probe — VOS executor is LIVE now
// ---------------------------------------------------------------------------

#[test]
fn readiness_probe_reports_vos_executor_ready() {
    let report = YydbSource::readiness();
    assert_eq!(report.backend_id, BACKEND_ID);
    assert!(report.schema_handshake_ready);
    assert!(report.vos_executor_ready);
    assert!(report.is_ready());
    assert!(report.message.contains("Connection"), "{}", report.message);
}

#[test]
fn ensure_schema_and_handshake_work_on_public_facade() {
    let db = YydbSource::open_in_memory().expect("open");
    assert_eq!(YydbSource::capabilities().backend_id, BACKEND_ID);

    db.ensure_schema(USER_SCHEMA).expect("schema");
    let hs = db.schema_handshake().expect("handshake");
    assert_eq!(hs.backend_id, BACKEND_ID);
    assert_eq!(hs.schema_version, Some(1));
    assert!(hs.has_document);
    assert!(hs.ddl_revision >= 1);
}

// ---------------------------------------------------------------------------
// Live VOS executor: query / execute / prepare / transaction
// ---------------------------------------------------------------------------

#[test]
fn vos_execute_inserts_a_row_and_query_finds_it() {
    let db = YydbSource::open_in_memory().unwrap();
    db.ensure_schema(USER_SCHEMA).unwrap();

    // DML via execute
    db.execute(
        r#"User {
            user_id: "550e8400-e29b-41d4-a716-446655440000",
            user_name: "ada",
            active: true,
        }.insert()"#,
    )
    .expect("insert");

    // Query via query
    let rows = db.query(r#"User.filter(x => true).collect()"#).expect("query");
    assert_eq!(rows.len(), 1);
    let first = &rows[0];
    assert_eq!(
        first.get("user_name").and_then(|v| match v {
            iris_types::Value::Str(s) => Some(s.as_str()),
            _ => None,
        }),
        Some("ada")
    );
    assert_eq!(first.get("active"), Some(&iris_types::Value::Bool(true)));
}

#[test]
fn vos_prepared_statement_executes_and_stale_checked() {
    let db = YydbSource::open_in_memory().unwrap();
    db.ensure_schema(USER_SCHEMA).unwrap();

    // Insert two users
    db.execute(r#"User { user_id: "11111111-1111-1111-1111-111111111111", user_name: "ada", active: true }.insert()"#).unwrap();
    db.execute(r#"User { user_id: "22222222-2222-2222-2222-222222222222", user_name: "bob", active: false }.insert()"#).unwrap();

    // Prepare a read plan
    let plan = db.prepare(r#"User.filter(x => x.active == true).collect()"#).expect("prepare");
    assert_eq!(plan.ddl_revision(), 1);

    let rows = plan.execute(&db).expect("execute");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].get("user_name").and_then(|v| match v {
            iris_types::Value::Str(s) => Some(s.as_str()),
            _ => None,
        }),
        Some("ada")
    );
}

#[test]
fn vos_prepared_stale_when_schema_drifts() {
    let db = YydbSource::open_in_memory().unwrap();
    db.ensure_schema(USER_SCHEMA).unwrap();

    let plan = db.prepare(r#"User.filter(x => true).collect()"#).unwrap();
    assert_eq!(plan.ddl_revision(), 1);

    // Directly mutate the schema stored in the YydbSource by re-opening and
    // using YYDB's migrate_schema — same semantics as a DDL revision bump.
    // We use YydbSource's escape hatch for this connector-local test since
    // migrate_schema hasn't been wrapped yet.
    let conn = db.connection();
    conn.migrate_schema(
        r#"
table User {
    @@user_id: utf8,
    @user_name: utf8,
    active: bool,
    extra: utf8,
}
"#,
        &Default::default(),
    )
    .unwrap();

    // ddl_revision now at least 2
    let hs = db.schema_handshake().unwrap();
    assert!(hs.ddl_revision > plan.ddl_revision());

    let err = plan.execute(&db).expect_err("stale plan must refuse");
    let msg = err.to_string();
    assert!(msg.contains("VOS-PREPARED-STALE"), "{msg}");
}

#[test]
fn vos_transaction_begin_commit_rollback() {
    let db = YydbSource::open_in_memory().unwrap();
    db.ensure_schema(USER_SCHEMA).unwrap();

    // begin → insert → commit
    db.begin().unwrap();
    db.execute(r#"User { user_id: "33333333-3333-3333-3333-333333333333", user_name: "charlie", active: true }.insert()"#).unwrap();
    db.commit().unwrap();

    let rows = db.query(r#"User.filter(x => x.user_name == "charlie").collect()"#).unwrap();
    assert_eq!(rows.len(), 1);

    // begin → insert → rollback → row disappears
    db.begin().unwrap();
    db.execute(r#"User { user_id: "44444444-4444-4444-4444-444444444444", user_name: "dave", active: true }.insert()"#).unwrap();
    db.rollback().unwrap();

    let rows = db.query(r#"User.filter(x => x.user_name == "dave").collect()"#).unwrap();
    assert_eq!(rows.len(), 0);
}

#[test]
fn reopen_preserves_schema_document() {
    let path = temp_db_path("reopen");
    {
        let db = YydbSource::open(&path).unwrap();
        db.ensure_schema(USER_SCHEMA).unwrap();
    }
    assert!(fs::metadata(&path).is_ok());
    let db = YydbSource::open(&path).unwrap();
    let hs = db.schema_handshake().unwrap();
    assert_eq!(hs.schema_version, Some(1));
    assert!(hs.has_document);

    let db = db.reopen().unwrap();
    let hs = db.schema_handshake().unwrap();
    assert_eq!(hs.schema_version, Some(1));
    let _ = fs::remove_file(&path);
}

// ---------------------------------------------------------------------------
// `bytes`: octets round trip through a real database file and a reopen
// ---------------------------------------------------------------------------

const BLOB_SCHEMA: &str = r#"
table Blob {
    @@blob_id: i64,
    label: utf8,
    body: bytes,
}
"#;

const BLOB_INSERT: &str = "Blob::insert({ blob_id: $blob_id, label: $label, body: $body })";

const BLOB_QUERY: &str = "Blob.filter(x => x.blob_id == $blob_id).collect()";

/// Payloads that a text path would corrupt, plus the boundaries where YYDB
/// spills `bytes` into object storage.
fn blob_cases() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("empty", Vec::new()),
        ("nul", vec![0x00, 0x00, 0x00]),
        ("invalid-utf8", vec![0xff, 0xfe, 0xc3, 0x28]),
        ("mixed", vec![0x00, 0xff, 0x41, 0xc3, 0x28]),
        ("all-256", (0..=255u8).collect()),
        ("4097", (0..=255u8).cycle().take(4097).collect()),
    ]
}

/// Tagged `bytes` parameter record — the shape the generated runtime sends.
fn tagged_write_params(blob_id: i64, label: &str, body: &[u8]) -> String {
    serde_json::json!({
        "blob_id": blob_id,
        "label": label,
        "body": { "$yydb": "bytes", "hex": iris_types::bytes_to_hex(body) },
    })
    .to_string()
}

/// Decode the tagged record into the typed parameter map a host binds.
fn typed_write_params(blob_id: i64, label: &str, body: &[u8]) -> BTreeMap<String, iris_types::Value> {
    let params: serde_json::Value = serde_json::from_str(&tagged_write_params(blob_id, label, body)).expect("params JSON");
    let object = params.as_object().expect("params object");
    let octets = iris_types::bytes_from_tagged_json(&object["body"]).expect("tagged `bytes`");
    BTreeMap::from([
        ("blob_id".to_string(), iris_types::Value::Int(object["blob_id"].as_i64().expect("blob_id"))),
        ("label".to_string(), iris_types::Value::Str(object["label"].as_str().expect("label").to_owned())),
        ("body".to_string(), iris_types::Value::Bytes(octets)),
    ])
}

fn read_body(db: &YydbSource, blob_id: i64) -> Vec<u8> {
    let named = BTreeMap::from([("blob_id".to_string(), iris_types::Value::Int(blob_id))]);
    let rows = db.query_with_params(BLOB_QUERY, &named).expect("read `body` back");
    assert_eq!(rows.len(), 1, "`blob_id` {blob_id} must read back exactly one row");
    match rows[0].get("body") {
        Some(iris_types::Value::Bytes(octets)) => octets.clone(),
        other => panic!("`bytes` must decode to octets, got {other:?}"),
    }
}

#[test]
fn bytes_round_trip_survives_close_and_reopen() {
    // `.yydx` selects the object-store layout, so payloads past
    // `INLINE_BYTES_MAX` spill to sibling blobs instead of being rejected.
    let path = temp_layout_path("bytes", "yydx");
    let db = YydbSource::open(&path).unwrap();
    db.ensure_schema(BLOB_SCHEMA).unwrap();

    let cases = blob_cases();
    db.begin().unwrap();
    for (index, (label, body)) in cases.iter().enumerate() {
        db.execute_with_params(BLOB_INSERT, &typed_write_params(index as i64 + 1, label, body))
            .unwrap_or_else(|error| panic!("insert `{label}`: {error}"));
    }
    db.commit().unwrap();

    for (index, (label, body)) in cases.iter().enumerate() {
        assert_eq!(read_body(&db, index as i64 + 1), *body, "`{label}` must read back byte for byte before reopen");
    }

    let db = db.reopen().expect("reopen");
    for (index, (label, body)) in cases.iter().enumerate() {
        assert_eq!(read_body(&db, index as i64 + 1), *body, "`{label}` must read back byte for byte after reopen");
    }

    drop(db);
    if let Some(dir) = path.parent() {
        let _ = fs::remove_dir_all(dir);
    }
}

#[test]
fn connector_does_not_depend_on_foreign_adapters() {
    let manifest = include_str!("../Cargo.toml");
    for banned in ["iris-adapter-mysql", "iris-adapter-postgres", "iris-adapter-sqlite", "iris-adapter-redis", "rusqlite", "sqlx"] {
        assert!(!manifest.contains(banned), "native YYDB connector must not depend on `{banned}`");
    }
    assert!(manifest.contains("yydb"));
}

// ---------------------------------------------------------------------------
// value_from_yydb — non-lossy conversion
// ---------------------------------------------------------------------------

use iris_connector_yydb::{row_from_yydb};

#[test]
fn yydb_date_time_timestamp_decimal_round_trip_to_iris() {
    // These variants were previously swallowed by the `_ => Null` wildcard.
    let date = yydb::Date::from_ymd(2026, 10, 29).expect("valid date");
    let time = yydb::Time::from_hms_nano(12, 34, 56, 789_000_000).expect("valid time");
    let ts = yydb::Timestamp::from_parts(date, time).expect("valid timestamp");
    let dec = yydb::Decimal::new(12345, 5, 2).expect("valid decimal");

    assert!(matches!(
        value_from_yydb(yydb::Value::Date(date)),
        Ok(iris_types::Value::Date(_))
    ));
    assert!(matches!(
        value_from_yydb(yydb::Value::Time(time)),
        Ok(iris_types::Value::Time(_))
    ));
    assert!(matches!(
        value_from_yydb(yydb::Value::Timestamp(ts)),
        Ok(iris_types::Value::DateTime(_))
    ));
    assert!(matches!(
        value_from_yydb(yydb::Value::Decimal(dec)),
        Ok(iris_types::Value::Decimal(_))
    ));
}

#[test]
fn yydb_object_still_fail_closed() {
    // The CAS `object` handle still has no Iris Phase 1 carrier and must not
    // silently degrade to Null. `vector` and `file` now round-trip through
    // a tagged JSON string, but the raw CAS handle remains fail-closed.
    let obj = yydb::ObjectRef {
        algo: yydb::HashAlgo::Blake3,
        hash: [0u8; 32],
        size: 0,
        kind: yydb::ObjectKind::VectorPayload,
    };

    let err = value_from_yydb(yydb::Value::Object(obj))
        .expect_err("CAS `object` must not silently map to Null");
    assert!(err.to_string().contains("object"));
}

#[test]
fn yydb_vector_round_trips_through_tagged_json_string() {
    // Iris has no Vector Value variant, so vector dimensions + components
    // round-trip through a single tagged JSON string inside Iris::Value::Str.
    let vec = yydb::Vector::new(std::sync::Arc::<[f32]>::from(vec![1.0_f32, 2.0, 3.0])).expect("vector");

    let iris_value = value_from_yydb(yydb::Value::Vector(vec.clone())).expect("Vector → Iris::Str");
    match &iris_value {
        iris_types::Value::Str(s) => {
            assert!(s.contains("__iris_vector__"), "serialized form must carry discriminating key: {s}");
        }
        other => panic!("expected Iris::Value::Str, got {other:?}"),
    }

    let mut named: BTreeMap<String, iris_types::Value> = BTreeMap::new();
    named.insert("v".into(), iris_value);
    let out = value_params_to_yydb(&named).expect("Iris params → YYDB binding");
    match out.get("v") {
        Some(yydb::Value::Vector(got)) => {
            assert_eq!(got.dim, vec.dim, "dimension must survive round-trip");
            assert_eq!(got.data.len(), vec.data.len());
            for (a, b) in got.data.iter().zip(vec.data.iter()) {
                assert_eq!(a.to_be_bytes(), b.to_be_bytes(), "component bit-pattern must survive round-trip");
            }
        }
        other => panic!("expected YydbValue::Vector, got {other:?}"),
    }
}

#[test]
fn tagged_vector_json_does_not_fool_plain_text_params() {
    // A plain Iris::Value::Str that does not carry our envelope key must stay
    // as YydbValue::Text — no accidental reinterpretation of existing code.
    let mut named: BTreeMap<String, iris_types::Value> = BTreeMap::new();
    named.insert("s".into(), iris_types::Value::Str("hello world".to_string()));
    let out = value_params_to_yydb(&named).expect("plain string → YYDB binding");
    assert_eq!(out.get("s"), Some(&yydb::Value::Text("hello world".to_string())));
}

#[test]
fn yydb_file_round_trips_through_tagged_json_string() {
    // Iris has no File Value variant yet, so chunk manifests round-trip as a
    // single tagged JSON string via Iris::Value::Str.
    let file = yydb::ChunkManifest {
        chunk_size: 4096,
        total_size: 12345,
        chunks: vec![yydb::ObjectRef {
            algo: yydb::HashAlgo::Blake3,
            hash: [0xAB; 32],
            size: 12345,
            kind: yydb::ObjectKind::Blob,
        }],
    };

    let iris_value = value_from_yydb(yydb::Value::File(file.clone())).expect("File → Iris::Str");
    match &iris_value {
        iris_types::Value::Str(s) => {
            assert!(s.contains("__iris_file__"), "serialized form must carry discriminating key: {s}");
        }
        other => panic!("expected Iris::Value::Str, got {other:?}"),
    }

    let mut named: BTreeMap<String, iris_types::Value> = BTreeMap::new();
    named.insert("f".into(), iris_value);
    let out = value_params_to_yydb(&named).expect("Iris params → YYDB binding");
    match out.get("f") {
        Some(yydb::Value::File(cm)) => {
            assert_eq!(cm.chunk_size, file.chunk_size);
            assert_eq!(cm.total_size, file.total_size);
            assert_eq!(cm.chunks.len(), file.chunks.len());
            assert_eq!(cm.chunks[0].hash, file.chunks[0].hash);
            assert_eq!(cm.chunks[0].size, file.chunks[0].size);
        }
        other => panic!("expected YydbValue::File, got {other:?}"),
    }
}

#[test]
fn tagged_file_json_does_not_fool_plain_text_params() {
    // A plain Iris::Value::Str that does not carry our envelope key must stay
    // as YydbValue::Text — no accidental reinterpretation of existing code.
    let mut named: BTreeMap<String, iris_types::Value> = BTreeMap::new();
    named.insert("s".into(), iris_types::Value::Str("hello world".to_string()));
    let out = value_params_to_yydb(&named).expect("plain string → YYDB binding");
    assert_eq!(out.get("s"), Some(&yydb::Value::Text("hello world".to_string())));
}

#[test]
fn row_from_yydb_propagates_unsupported_cell_error() {
    // BTreeMap<String, YydbValue> containing a CAS Object cell must fail the
    // row conversion instead of silently dropping the cell to Null.
    let obj = yydb::ObjectRef {
        algo: yydb::HashAlgo::Blake3,
        hash: [0u8; 32],
        size: 0,
        kind: yydb::ObjectKind::Blob,
    };
    let mut row: BTreeMap<String, yydb::Value> = BTreeMap::new();
    row.insert("id".to_string(), yydb::Value::I64(1));
    row.insert("payload".to_string(), yydb::Value::Object(obj));

    let err = row_from_yydb(row).expect_err("row with CAS `object` cell must fail");
    let msg = err.to_string();
    assert!(msg.contains("object"), "message must name the offending variant: {msg}");
}

#[test]
fn row_from_yydb_vector_cell_round_trips_via_tagged_string() {
    // A row cell containing a Vector now round-trips as a tagged JSON string
    // inside Iris::Value::Str instead of failing.
    let vec = yydb::Vector::new(std::sync::Arc::<[f32]>::from(vec![0.5_f32, -1.5, 2.0])).expect("vector");
    let mut row: BTreeMap<String, yydb::Value> = BTreeMap::new();
    row.insert("id".to_string(), yydb::Value::I64(1));
    row.insert("embedding".to_string(), yydb::Value::Vector(vec));

    let iris_row = row_from_yydb(row).expect("vector cell must convert");
    assert_eq!(iris_row.get("id"), Some(&iris_types::Value::Int(1)));
    match iris_row.get("embedding") {
        Some(iris_types::Value::Str(s)) => {
            assert!(s.contains("__iris_vector__"), "must be tagged vector string: {s}");
        }
        other => panic!("expected tagged string, got {other:?}"),
    }
}

#[test]
fn row_from_yydb_nested_row_is_still_ok() {
    // Nested YYDB `Row` correctly lifts to Iris `Value::Object`.
    let mut inner: BTreeMap<String, yydb::Value> = BTreeMap::new();
    inner.insert("name".to_string(), yydb::Value::Text("nested".to_string()));

    let mut outer: BTreeMap<String, yydb::Value> = BTreeMap::new();
    outer.insert("id".to_string(), yydb::Value::I64(1));
    outer.insert("profile".to_string(), yydb::Value::Row(inner));

    let iris_row = row_from_yydb(outer).expect("nested row must convert");
    assert_eq!(iris_row.get("id"), Some(&iris_types::Value::Int(1)));
    match iris_row.get("profile") {
        Some(iris_types::Value::Object(map)) => {
            assert_eq!(map.get("name"), Some(&iris_types::Value::Str("nested".to_string())));
        }
        other => panic!("expected Object, got {other:?}"),
    }
}
