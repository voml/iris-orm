//! Gate: `iris-wasm` must compile for `wasm32-unknown-unknown` and emit `.wasm`.

use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn wasm_artifact() -> PathBuf {
    workspace_root().join("target/wasm32-unknown-unknown/release/iris_wasm.wasm")
}

#[test]
fn release_wasm32_target_builds() {
    let workspace = workspace_root();
    let status = Command::new("cargo")
        .current_dir(&workspace)
        .args([
            "build",
            "-p",
            "iris-wasm",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
        ])
        .status()
        .expect("spawn cargo");
    assert!(status.success(), "wasm32 release build failed");
}

#[test]
fn release_wasm32_artifact_exists_with_sane_size() {
    release_wasm32_target_builds();
    let wasm = wasm_artifact();
    assert!(
        wasm.is_file(),
        "missing wasm artifact at {}",
        wasm.display()
    );
    let bytes = std::fs::metadata(&wasm)
        .expect("stat wasm")
        .len();
    assert!(bytes > 8_192, "wasm artifact suspiciously small: {bytes} bytes");
    // Browser bundle budget until dependency slimming lands (iris-generator + vos/dejavu chain).
    assert!(
        bytes < 2_000_000,
        "wasm artifact too large for browser target: {bytes} bytes"
    );
}
