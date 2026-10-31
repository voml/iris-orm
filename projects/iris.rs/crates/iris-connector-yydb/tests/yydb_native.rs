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

use iris_connector_yydb::row_from_yydb;

#[test]
fn yydb_date_time_timestamp_decimal_round_trip_to_iris() {
    // These variants were previously swallowed by the `_ => Null` wildcard.
    let date = yydb::Date::from_ymd(2026, 10, 29).expect("valid date");
    let time = yydb::Time::from_hms_nano(12, 34, 56, 789_000_000).expect("valid time");
    let ts = yydb::Timestamp::from_parts(date, time).expect("valid timestamp");
    let dec = yydb::Decimal::new(12345, 5, 2).expect("valid decimal");

    assert!(matches!(value_from_yydb(yydb::Value::Date(date)), Ok(iris_types::Value::Date(_))));
    assert!(matches!(value_from_yydb(yydb::Value::Time(time)), Ok(iris_types::Value::Time(_))));
    assert!(matches!(value_from_yydb(yydb::Value::Timestamp(ts)), Ok(iris_types::Value::DateTime(_))));
    assert!(matches!(value_from_yydb(yydb::Value::Decimal(dec)), Ok(iris_types::Value::Decimal(_))));
}

#[test]
fn yydb_object_still_fail_closed() {
    // The CAS `object` handle still has no Iris carrier and must not silently
    // degrade to Null. `vector` and `file` round-trip as native typed values,
    // but the raw CAS handle remains fail-closed.
    let obj = yydb::ObjectRef { algo: yydb::HashAlgo::Blake3, hash: [0u8; 32], size: 0, kind: yydb::ObjectKind::VectorPayload };

    let err = value_from_yydb(yydb::Value::Object(obj)).expect_err("CAS `object` must not silently map to Null");
    assert!(err.to_string().contains("object"));
}

#[test]
fn yydb_vector_round_trips_as_native_typed_value() {
    // `vector` keeps its native identity end to end: dimension and component
    // bit patterns survive both conversion directions, never a text form.
    let vec = yydb::Vector::new(std::sync::Arc::<[f32]>::from(vec![1.0_f32, 2.0, 3.0])).expect("vector");

    let iris_value = value_from_yydb(yydb::Value::Vector(vec.clone())).expect("Vector → Iris::Vector");
    match &iris_value {
        iris_types::Value::Vector(v) => {
            assert_eq!(v.dimension, vec.dim, "dimension must survive conversion");
            assert_eq!(v.components, vec.data.to_vec(), "components must survive conversion");
        }
        other => panic!("expected Iris::Value::Vector, got {other:?}"),
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
fn plain_text_params_are_never_reinterpreted_as_payloads() {
    // `Value::Str` always binds as `YydbValue::Text`, even when the text
    // spells a legacy envelope key or a canonical tagged JSON payload: the
    // typed variants carry payloads, text never gets reinterpreted.
    let cases = [
        "hello world",
        "__iris_vector__{\"dimension\":2}",
        "__iris_file__{\"chunkSize\":4096}",
        r#"{"$yydb":"vector","dimension":2,"components":"0000803f00000040"}"#,
        r#"{"$yydb":"file","digest":"a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3","octets":10,"manifest":{"hash":"b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4","size":128}}"#,
    ];
    let mut named: BTreeMap<String, iris_types::Value> = BTreeMap::new();
    for case in cases {
        named.insert("s".into(), iris_types::Value::Str(case.to_string()));
        let out = value_params_to_yydb(&named).expect("plain string → YYDB binding");
        assert_eq!(out.get("s"), Some(&yydb::Value::Text(case.to_string())), "text `{case}` must bind as text");
    }
}

#[test]
fn yydb_file_round_trips_as_native_typed_value() {
    // `file` keeps its native descriptor identity end to end: digest, octets,
    // metadata and the manifest reference survive both conversion directions.
    let file = yydb::FileDescriptor {
        digest: "a3".repeat(32),
        octets: 12345,
        name: Some("note.bin".into()),
        media_type: Some("application/octet-stream".into()),
        manifest: yydb::FileManifestRef { hash: [0xAB; 32], size: 128 },
    };

    let iris_value = value_from_yydb(yydb::Value::File(file.clone())).expect("File → Iris::File");
    match &iris_value {
        iris_types::Value::File(f) => {
            assert_eq!(f.digest, file.digest, "digest must survive conversion");
            assert_eq!(f.octets, file.octets, "octets must survive conversion");
            assert_eq!(f.name, file.name, "name must survive conversion");
            assert_eq!(f.media_type, file.media_type, "media type must survive conversion");
            assert_eq!(f.manifest.hash, file.manifest.hash_hex(), "manifest hash must survive as canonical lowercase hex");
            assert_eq!(f.manifest.size, file.manifest.size, "manifest size must survive conversion");
        }
        other => panic!("expected Iris::Value::File, got {other:?}"),
    }

    let mut named: BTreeMap<String, iris_types::Value> = BTreeMap::new();
    named.insert("f".into(), iris_value);
    let out = value_params_to_yydb(&named).expect("Iris params → YYDB binding");
    match out.get("f") {
        Some(yydb::Value::File(got)) => assert_eq!(got, &file, "descriptor must survive round-trip"),
        other => panic!("expected YydbValue::File, got {other:?}"),
    }
}

#[test]
fn row_from_yydb_propagates_unsupported_cell_error() {
    // BTreeMap<String, YydbValue> containing a CAS Object cell must fail the
    // row conversion instead of silently dropping the cell to Null.
    let obj = yydb::ObjectRef { algo: yydb::HashAlgo::Blake3, hash: [0u8; 32], size: 0, kind: yydb::ObjectKind::Blob };
    let mut row: BTreeMap<String, yydb::Value> = BTreeMap::new();
    row.insert("id".to_string(), yydb::Value::I64(1));
    row.insert("payload".to_string(), yydb::Value::Object(obj));

    let err = row_from_yydb(row).expect_err("row with CAS `object` cell must fail");
    let msg = err.to_string();
    assert!(msg.contains("object"), "message must name the offending variant: {msg}");
}

#[test]
fn row_from_yydb_vector_cell_round_trips_as_native_value() {
    // A row cell containing a Vector lands in Iris::Value::Vector with its
    // dimension and components intact, never as a tagged string.
    let vec = yydb::Vector::new(std::sync::Arc::<[f32]>::from(vec![0.5_f32, -1.5, 2.0])).expect("vector");
    let mut row: BTreeMap<String, yydb::Value> = BTreeMap::new();
    row.insert("id".to_string(), yydb::Value::I64(1));
    row.insert("embedding".to_string(), yydb::Value::Vector(vec.clone()));

    let iris_row = row_from_yydb(row).expect("vector cell must convert");
    assert_eq!(iris_row.get("id"), Some(&iris_types::Value::Int(1)));
    match iris_row.get("embedding") {
        Some(iris_types::Value::Vector(v)) => {
            assert_eq!(v.dimension, vec.dim);
            assert_eq!(v.components, vec.data.to_vec());
        }
        other => panic!("expected native vector, got {other:?}"),
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

// ---------------------------------------------------------------------------
// `vector` / `file`: typed payloads survive a real `.yydx` close and reopen
// ---------------------------------------------------------------------------

/// Inline row-codec vector (≤ 120 components), plus a CAS-spilled column
/// beyond the inline threshold that only the `.yydx` layout persists.
const EMB_SCHEMA: &str = r#"
table Emb {
    @@emb_id: i64,
    label: utf8,
    embedding: vector<3>,
}

table BigEmb {
    @@big_emb_id: i64,
    embedding: vector<121>,
}
"#;

const EMB_INSERT: &str = "Emb::insert({ emb_id: $emb_id, label: $label, embedding: $embedding })";

const EMB_QUERY: &str = "Emb.filter(x => x.emb_id == $emb_id).collect()";

const BIG_EMB_INSERT: &str = "BigEmb::insert({ big_emb_id: $big_emb_id, embedding: $embedding })";

const BIG_EMB_QUERY: &str = "BigEmb.filter(x => x.big_emb_id == $big_emb_id).collect()";

const ASSET_SCHEMA: &str = r#"
table Asset {
    @@asset_id: i64,
    payload: file,
}
"#;

const ASSET_INSERT: &str = "Asset::insert({ asset_id: $asset_id, payload: $payload })";

const ASSET_QUERY: &str = "Asset.filter(x => x.asset_id == $asset_id).collect()";

fn read_embedding(db: &YydbSource, emb_id: i64) -> iris_types::VectorValue {
    let named = BTreeMap::from([("emb_id".to_string(), iris_types::Value::Int(emb_id))]);
    let rows = db.query_with_params(EMB_QUERY, &named).expect("read `embedding` back");
    assert_eq!(rows.len(), 1, "`emb_id` {emb_id} must read back exactly one row");
    match rows[0].get("embedding") {
        Some(iris_types::Value::Vector(v)) => v.clone(),
        other => panic!("`vector` must decode to a native typed value, got {other:?}"),
    }
}

fn assert_components_exact(got: &iris_types::VectorValue, expected: &[f32], context: &str) {
    assert_eq!(got.dimension as usize, expected.len(), "{context}: dimension must survive");
    assert_eq!(got.components.len(), expected.len(), "{context}: component count must survive");
    for (index, (a, b)) in got.components.iter().zip(expected.iter()).enumerate() {
        assert_eq!(a.to_bits(), b.to_bits(), "{context}: component {index} bit pattern must survive exactly");
    }
}

#[test]
fn vector_round_trip_survives_close_and_reopen() {
    // `.yydx` keeps the CAS object store persistent, so both the inline row
    // codec (vector<3>) and the spilled payload (vector<121>) must reopen.
    let path = temp_layout_path("vector", "yydx");
    let db = YydbSource::open(&path).unwrap();
    db.ensure_schema(EMB_SCHEMA).unwrap();

    // Bit patterns a text path would corrupt: negative zero, the smallest
    // subnormal, and the largest finite f32.
    let small = vec![-0.0_f32, f32::from_bits(0x0000_0001), f32::MAX];
    let large: Vec<f32> = (0..121).map(|index| index as f32 * 0.5 + 0.25).collect();

    db.begin().unwrap();
    db.execute_with_params(
        EMB_INSERT,
        &BTreeMap::from([
            ("emb_id".to_string(), iris_types::Value::Int(1)),
            ("label".to_string(), iris_types::Value::Str("exact-bits".into())),
            ("embedding".to_string(), iris_types::Value::Vector(iris_types::VectorValue::new(3, small.clone()).unwrap())),
        ]),
    )
    .unwrap();
    db.execute_with_params(
        BIG_EMB_INSERT,
        &BTreeMap::from([
            ("big_emb_id".to_string(), iris_types::Value::Int(1)),
            ("embedding".to_string(), iris_types::Value::Vector(iris_types::VectorValue::new(121, large.clone()).unwrap())),
        ]),
    )
    .unwrap();
    db.commit().unwrap();

    assert_components_exact(&read_embedding(&db, 1), &small, "inline `vector<3>` before reopen");
    let named = BTreeMap::from([("big_emb_id".to_string(), iris_types::Value::Int(1))]);
    let rows = db.query_with_params(BIG_EMB_QUERY, &named).expect("read spilled `embedding` back");
    match rows[0].get("embedding") {
        Some(iris_types::Value::Vector(v)) => assert_components_exact(v, &large, "CAS-spilled `vector<121>` before reopen"),
        other => panic!("`vector` must decode to a native typed value, got {other:?}"),
    }

    let db = db.reopen().expect("reopen");
    assert_components_exact(&read_embedding(&db, 1), &small, "inline `vector<3>` after reopen");
    let named = BTreeMap::from([("big_emb_id".to_string(), iris_types::Value::Int(1))]);
    let rows = db.query_with_params(BIG_EMB_QUERY, &named).expect("read spilled `embedding` after reopen");
    match rows[0].get("embedding") {
        Some(iris_types::Value::Vector(v)) => assert_components_exact(v, &large, "CAS-spilled `vector<121>` after reopen"),
        other => panic!("`vector` must decode to a native typed value, got {other:?}"),
    }

    drop(db);
    if let Some(dir) = path.parent() {
        let _ = fs::remove_dir_all(dir);
    }
}

#[test]
fn file_descriptor_and_content_survive_close_and_reopen() {
    let path = temp_layout_path("file", "yydx");
    let db = YydbSource::open(&path).unwrap();
    db.ensure_schema(ASSET_SCHEMA).unwrap();

    // Chunked content with bytes a text carrier could never carry.
    let octets: Vec<u8> = (0..700u32).map(|index| (index % 256) as u8).collect();
    let stored = db
        .connection()
        .put_file(std::io::Cursor::new(octets.clone()), 256, Some("note.bin".into()), Some("application/octet-stream".into()))
        .expect("put_file");

    let descriptor = match iris_connector_yydb::value_from_yydb(yydb::Value::File(stored.clone())).expect("typed `file`") {
        iris_types::Value::File(f) => f,
        other => panic!("expected `Value::File`, got {other:?}"),
    };

    db.execute_with_params(
        ASSET_INSERT,
        &BTreeMap::from([
            ("asset_id".to_string(), iris_types::Value::Int(1)),
            ("payload".to_string(), iris_types::Value::File(descriptor.clone())),
        ]),
    )
    .unwrap();

    let read_asset = |db: &YydbSource, context: &str| -> iris_types::FileDescriptor {
        let named = BTreeMap::from([("asset_id".to_string(), iris_types::Value::Int(1))]);
        let rows = db.query_with_params(ASSET_QUERY, &named).unwrap_or_else(|e| panic!("{context}: query: {e}"));
        assert_eq!(rows.len(), 1, "{context}: exactly one row");
        match rows[0].get("payload") {
            Some(iris_types::Value::File(f)) => f.clone(),
            other => panic!("{context}: `file` must decode to a native descriptor, got {other:?}"),
        }
    };

    for (context, got) in [("before reopen", read_asset(&db, "before reopen")), ("after reopen", {
        let db = db.reopen().expect("reopen");
        let got = read_asset(&db, "after reopen");
        // Content bytes must still read through the reopened object store.
        assert_eq!(db.connection().read_file(&stored).expect("read_file after reopen"), octets, "file content after reopen");
        drop(db);
        got
    })] {
        assert_eq!(got, descriptor, "{context}: descriptor fields must survive verbatim");
        assert_eq!(got.digest, stored.digest, "{context}: digest must survive");
        assert_eq!(got.octets, octets.len() as u64, "{context}: octet count must survive");
    }

    if let Some(dir) = path.parent() {
        let _ = fs::remove_dir_all(dir);
    }
}

// ---------------------------------------------------------------------------
// Converged adapter surface — `execute_plan` / `insert` / transactions
// ---------------------------------------------------------------------------

fn blob_row_write(blob_id: i64, octets: Vec<u8>) -> iris_types::RowWrite {
    iris_types::RowWrite {
        table: "Blob".into(),
        primary_key: "blob_id".into(),
        fields: BTreeMap::from([
            ("blob_id".to_string(), iris_types::Value::Int(blob_id)),
            ("label".to_string(), iris_types::Value::Str(format!("surface-{blob_id}"))),
            ("body".to_string(), iris_types::Value::Bytes(octets)),
        ]),
    }
}

#[test]
fn adapter_execute_plan_runs_planned_reads() {
    let db = YydbSource::open_in_memory().unwrap();
    db.ensure_schema(USER_SCHEMA).unwrap();
    db.execute(r#"User { user_id: "11111111-1111-1111-1111-111111111111", user_name: "ada", active: true }.insert()"#).unwrap();
    db.execute(r#"User { user_id: "22222222-2222-2222-2222-222222222222", user_name: "bob", active: false }.insert()"#).unwrap();

    let planner = iris_types::Planner::new(YydbSource::capabilities());

    let plan = planner.plan_source(r#"User.filter(x => x.active == true).collect()"#).unwrap();
    let rows = db.execute_plan(&plan).expect("planned `FieldBool` read");
    assert_eq!(rows.len(), 1);

    let plan = planner.plan_source(r#"User.filter(x => x.user_name == "bob").collect()"#).unwrap();
    let rows = db.execute_plan(&plan).expect("planned string compare binds a typed `$param`");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("user_name"), Some(&iris_types::Value::Str("bob".into())));

    let plan = planner
        .plan_source(r#"User.filter(x => x.active == true && x.user_name == "ada").sort_by(x => x.user_name).skip(0).take(5).map(x => x.{ user_name, }).collect()"#)
        .expect("plan compound read");
    let rows = db.execute_plan(&plan).expect("planned and/sort/skip/take/project read");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("user_name"), Some(&iris_types::Value::Str("ada".into())));
}

#[test]
fn adapter_execute_plan_runs_planned_insert() {
    let db = YydbSource::open_in_memory().unwrap();
    db.ensure_schema(USER_SCHEMA).unwrap();
    let planner = iris_types::Planner::new(YydbSource::capabilities());
    let plan = planner
        .plan_source(r#"User::insert({ user_id: "33333333-3333-3333-3333-333333333333", user_name: "charlie", active: true })"#)
        .expect("plan insert");
    let rows = db.execute_plan(&plan).expect("planned insert");
    assert!(rows.is_empty(), "writes return no rows");
    let found = db.query(r#"User.filter(x => x.user_name == "charlie").collect()"#).unwrap();
    assert_eq!(found.len(), 1);
}

#[test]
fn adapter_insert_row_write_carries_typed_payloads() {
    let path = temp_layout_path("row-write", "yydx");
    let db = YydbSource::open(&path).unwrap();
    db.ensure_schema(BLOB_SCHEMA).unwrap();
    db.insert(&blob_row_write(1, vec![0x00, 0xff, 0x41])).expect("typed `RowWrite` insert");
    assert_eq!(read_body(&db, 1), vec![0x00, 0xff, 0x41]);
    drop(db);
    if let Some(dir) = path.parent() {
        let _ = fs::remove_dir_all(dir);
    }
}

#[test]
fn adapter_update_and_delete_fail_closed() {
    let db = YydbSource::open_in_memory().unwrap();
    db.ensure_schema(BLOB_SCHEMA).unwrap();
    let err = db.update(&blob_row_write(1, vec![0x01])).expect_err("VOS writes are insert-only");
    assert!(err.to_string().contains("insert-only"), "{err}");
    let err = db.delete("Blob", "blob_id", &iris_types::Value::Int(1)).expect_err("VOS writes are insert-only");
    assert!(err.to_string().contains("insert-only"), "{err}");
}

#[test]
fn adapter_transaction_commits_and_with_rollback_leaves_no_residue() {
    let path = temp_layout_path("txn-surface", "yydx");
    let db = YydbSource::open(&path).unwrap();
    db.ensure_schema(BLOB_SCHEMA).unwrap();

    db.transaction(|conn| db.insert_on(conn, &blob_row_write(1, vec![0xAA]))).expect("commit");
    assert_eq!(read_body(&db, 1), vec![0xAA]);

    let err = db
        .with_rollback(|conn| {
            db.insert_on(conn, &blob_row_write(2, vec![0xBB]))?;
            Err::<(), _>(iris_connector_yydb::Error::Runtime("rollback probe".into()))
        })
        .expect_err("closure error must propagate");
    assert!(err.to_string().contains("rollback probe"), "{err}");

    let named = BTreeMap::from([("blob_id".to_string(), iris_types::Value::Int(2))]);
    let rows = db.query_with_params(BLOB_QUERY, &named).expect("query after rollback");
    assert_eq!(rows.len(), 0, "rolled-back insert must leave no residue");

    drop(db);
    if let Some(dir) = path.parent() {
        let _ = fs::remove_dir_all(dir);
    }
}
