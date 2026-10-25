//! Regenerate the host-neutral runtime modules in both TypeScript host packages.
//!
//! The modules come from `templates/typescript/runtime/` through
//! `iris_generator::emit_typescript_runtime`. Run from anywhere in the workspace:
//!
//! ```text
//! cargo run -p iris-generator --bin iris-runtime-sync
//! ```

use std::path::PathBuf;

use iris_generator::write_typescript_runtime;

/// Package runtime directories that receive the shared modules.
const RUNTIME_DIRS: &[&str] = &["projects/iris.ts/packages/iris/src/runtime", "projects/iris.ts/packages/iris-language/src/runtime"];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../..").canonicalize()?;
    for relative in RUNTIME_DIRS {
        let dir = repo_root.join(relative);
        let written = write_typescript_runtime(&dir)?;
        println!("wrote {} runtime modules to {}", written.len(), dir.display());
    }
    Ok(())
}
