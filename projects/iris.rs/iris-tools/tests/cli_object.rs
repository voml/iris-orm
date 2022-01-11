//! Phase 10-E CLI: object status / gc for FsObjectStore.

use std::process::Command;

use iris::{FsObjectStore, OBJECT_HASH_ALG_BLAKE3, ObjectId, ObjectPolicy};

fn iris_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_iris"))
}

#[test]
fn object_status_and_gc_cli() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("iris-obj-cli-{stamp}"));
    let store = FsObjectStore::open(
        &root,
        ObjectPolicy {
            hash_alg: Some(OBJECT_HASH_ALG_BLAKE3.into()),
            orphan_ttl_secs: Some(0),
            pending_ttl_secs: Some(0),
        },
    )
    .unwrap();

    let id = ObjectId::new("cli-obj").unwrap();
    store.begin_pending(id.clone(), 1).unwrap();
    store.write_pending(&id, b"payload", 2).unwrap();
    store.abort(&id, 3).unwrap();
    assert!(store.committed_reference(&id).unwrap().is_none());

    let status_out = root.join("status.von");
    let out = iris_bin()
        .args([
            "object",
            "status",
            "--root",
            root.to_str().unwrap(),
            "--out",
            status_out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let body = std::fs::read_to_string(&status_out).unwrap();
    assert!(body.contains("iris.object_store_status"));
    assert!(body.contains("aborted") || body.contains("cli-obj"));
    assert!(!body.to_ascii_lowercase().contains("select "));
    assert!(!body.to_ascii_lowercase().contains("password="));

    let out = iris_bin()
        .args(["object", "gc", "--root", root.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("--yes"));

    let out = iris_bin()
        .args(["object", "gc", "--root", root.to_str().unwrap(), "--yes"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("removed="));
    assert!(store.meta(&id).is_err());

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn help_lists_object() {
    let out = iris_bin().arg("--help").output().unwrap();
    assert!(out.status.success());
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(help.contains("object"));
}
