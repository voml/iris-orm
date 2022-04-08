//! TypeScript generated-client emitter (Dejavu templates only).

use std::path::{Path, PathBuf};

use crate::{GenerationModel, Result, render};

/// Emit all TypeScript client files for a generation model.
pub fn emit_typescript_client(model: &GenerationModel) -> Result<Vec<(String, String)>> {
    let ctx = model.typescript_template_context();

    let mut files: Vec<(String, String)> = vec![
        ("metadata.ts".into(), render("typescript/metadata", &ctx)?),
        ("index.ts".into(), render("typescript/index", &ctx)?),
        ("node.ts".into(), render("typescript/node", &ctx)?),
        ("browser.ts".into(), render("typescript/browser", &ctx)?),
        ("_internal/synthesize.ts".into(), render("typescript/synthesize", &ctx)?),
        ("references.ts".into(), render("typescript/references", &ctx)?),
        ("models.ts".into(), render("typescript/models", &ctx)?),
        ("inputs.ts".into(), render("typescript/inputs", &ctx)?),
        ("operations.ts".into(), render("typescript/operations", &ctx)?),
        ("errors.ts".into(), render("typescript/errors", &ctx)?),
    ];

    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}

/// Write TypeScript client files into `{out_dir}/src/generated/iris/`.
pub fn write_typescript_client(model: &GenerationModel, out_dir: &Path) -> Result<Vec<PathBuf>> {
    let root = crate::typescript_target_dir(out_dir);
    std::fs::create_dir_all(root.join("_internal"))?;
    let mut written = Vec::new();
    for (name, content) in emit_typescript_client(model)? {
        let target = root.join(&name);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp_name = name.replace('/', "__");
        let tmp = root.join(format!("{tmp_name}.tmp"));
        std::fs::write(&tmp, content)?;
        std::fs::rename(&tmp, &target)?;
        written.push(target);
    }
    Ok(written)
}

/// Unsupported generate target (caller-facing).
pub fn unsupported_target(target: &str) -> crate::Error {
    crate::Error::UnsupportedTarget(target.to_owned())
}
