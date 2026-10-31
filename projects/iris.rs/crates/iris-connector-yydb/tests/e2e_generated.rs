//! Task 1 end-to-end evidence: the checked-in generated `yydb` client and
//! direct connector calls run the same VOS programs on a real database file.
//!
//! `tests/generated/` is the committed generator output for `schema.vos`;
//! the drift gate below refuses any divergence, and the live test drives
//! ordinary fields plus `bytes` / `vector<4>` / `file` payloads through the
//! generated delegates, a generated transaction, the direct escape hatch,
//! a close-and-reopen, and the fail-closed write paths.

use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use iris::{FileDescriptor, FileManifestRef, Planner, Value};
use iris_connector_yydb::YydbSource;

// The checked-in fixture is generator output; this test exercises part of its
// surface, so unused delegate items are expected.
#[allow(dead_code, unused_imports)]
#[path = "generated/mod.rs"]
mod generated;

use generated::{Db, Doc, DocWhere};

const SCHEMA: &str = include_str!("generated/schema.vos");

fn temp_layout_path(label: &str, extension: &str) -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("iris-yydb-e2e-{label}-{nanos}"));
    fs::create_dir_all(&dir).expect("temp dir");
    dir.join(format!("db.{extension}"))
}

fn cleanup(path: &Path) {
    if let Some(dir) = path.parent() {
        let _ = fs::remove_dir_all(dir);
    }
}

#[test]
fn checked_in_generated_fixture_matches_the_generator() {
    let model = iris_generator::GenerationModel::from_vos_schema(SCHEMA).expect("schema");
    let files = iris_generator::emit_rust_files(&model, "yydb").expect("emit");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/generated");
    for (name, content) in &files {
        let checked_in = fs::read_to_string(dir.join(name)).unwrap_or_else(|error| panic!("fixture `{name}` missing: {error}"));
        // `core.autocrlf` decides each side's line endings independently of the
        // generator's logic; the gate is about content drift, not line endings.
        assert_eq!(checked_in.replace("\r\n", "\n"), content.replace("\r\n", "\n"), "fixture `{name}` drifted from generator output");
    }
}

fn stored_descriptor(db: &YydbSource, octets: &[u8]) -> FileDescriptor {
    let stored = db
        .connection()
        .put_file(Cursor::new(octets.to_vec()), 256, Some("note.bin".into()), Some("application/octet-stream".into()))
        .expect("put_file");
    match iris_connector_yydb::value_from_yydb(yydb::Value::File(stored)).expect("typed `file`") {
        Value::File(descriptor) => descriptor,
        other => panic!("expected `Value::File`, got {other:?}"),
    }
}

#[test]
fn generated_client_and_direct_calls_close_the_loop_on_a_real_database() {
    // `.yydx` keeps the object store persistent so the `file` payload and its
    // content survive the reopen leg of this test.
    let path = temp_layout_path("generated", "yydx");
    let db = YydbSource::open(&path).unwrap();
    db.ensure_schema(SCHEMA).unwrap();
    let client = Db::new(&db);

    let octets: Vec<u8> = (0..700u32).map(|index| (index % 256) as u8).collect();
    let attachment = stored_descriptor(&db, &octets);
    let embedding = vec![-0.0_f32, f32::from_bits(0x0000_0001), 1.5, f32::MAX];

    let row = Doc {
        doc_id: 1,
        title: "alpha".into(),
        published: true,
        body: vec![0x00, 0xff, 0xc3, 0x28],
        embedding: embedding.clone(),
        attachment: attachment.clone(),
    };
    client.doc().insert(&row).expect("generated insert");

    let found = client
        .doc()
        .find_unique(&DocWhere { doc_id: Some(1), ..Default::default() })
        .expect("find_unique")
        .expect("row");
    assert_eq!(found, row, "generated read-back must reproduce the typed payload row");

    let row2 =
        Doc { doc_id: 2, title: "beta".into(), published: false, body: vec![0x01], embedding: embedding.clone(), attachment: attachment.clone() };
    client
        .transaction(|txn| {
            txn.doc().insert(&row2)?;
            let inside = txn.doc().find_many(&DocWhere { title: Some("beta".into()), ..Default::default() })?;
            assert_eq!(inside.len(), 1, "the row is visible inside its own transaction");
            Ok(())
        })
        .expect("transaction commit");

    let direct = db.query("Doc.filter(x => x.published == true).collect()").expect("direct call");
    let via_client = client.query("Doc.filter(x => x.published == true).collect()").expect("generated escape hatch");
    assert_eq!(direct, via_client, "both paths decode the same rows");
    assert_eq!(via_client.len(), 1);

    let planner = Planner::new(YydbSource::capabilities());
    let plain = planner.plan_source("Doc.filter(x => x.published == true).collect()").expect("plan plain");
    let spaced = planner.plan_source("Doc.filter( x => x.published == true ).collect()").expect("plan spaced");
    assert_eq!(plain.envelope.semantic_hash, spaced.envelope.semantic_hash, "identity is content-addressed, not text-addressed");
    let rows = db.execute_plan(&spaced).expect("planned spaced read");
    assert_eq!(rows, direct, "the planned path returns the same rows as the direct call");

    drop(client);
    let db = db.reopen().expect("reopen");
    let client = Db::new(&db);
    let found = client
        .doc()
        .find_unique(&DocWhere { doc_id: Some(1), ..Default::default() })
        .expect("read after reopen")
        .expect("row after reopen");
    assert_eq!(found.body, row.body, "`bytes` must survive the reopen byte for byte");
    for (index, (got, want)) in found.embedding.iter().zip(row.embedding.iter()).enumerate() {
        assert_eq!(got.to_bits(), want.to_bits(), "component {index} bit pattern must survive the reopen");
    }
    assert_eq!(found.attachment, row.attachment, "`file` descriptor must survive the reopen verbatim");

    let err = client.doc().update(&row).expect_err("VOS writes are insert-only");
    assert!(err.to_string().contains("insert-only"), "{err}");
    let err = client.doc().delete(&Value::Int(1)).expect_err("VOS writes are insert-only");
    assert!(err.to_string().contains("insert-only"), "{err}");

    drop(client);
    drop(db);
    cleanup(&path);
}

#[test]
fn generated_write_rejects_a_wrong_dimension_vector_before_talking_to_the_backend() {
    let db = YydbSource::open_in_memory().unwrap();
    db.ensure_schema(SCHEMA).unwrap();
    let client = Db::new(&db);
    let row = Doc {
        doc_id: 1,
        title: "short".into(),
        published: true,
        body: Vec::new(),
        embedding: vec![1.0, 2.0],
        attachment: FileDescriptor::new(
            "a3".repeat(32),
            256,
            Some("note.bin".into()),
            Some("application/octet-stream".into()),
            FileManifestRef { hash: "b4".repeat(32), size: 128 },
        )
        .expect("canonical descriptor"),
    };
    let err = client.doc().insert(&row).expect_err("a `vector<4>` row cannot carry two components");
    assert!(err.to_string().contains("dimension"), "{err}");
}
