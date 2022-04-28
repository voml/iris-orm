//! TypeScript generated-client emitter (Dejavu templates only).

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::{GenerationModel, Result, render, unescape_dejavu_template};

fn render_ts(name: &str, ctx: &Value) -> Result<String> {
    let rendered = unescape_dejavu_template(&render(name, ctx)?);
    Ok(normalize_ts_blank_lines(&rendered))
}

/// Dejavu templates interleave blank lines for readability; compact before write.
fn normalize_ts_blank_lines(text: &str) -> String {
    let mut normalized = text.replace("\r\n", "\n").trim().to_string();
    while normalized.contains("\n\n") {
        normalized = normalized.replace("\n\n", "\n");
    }
    normalized.push('\n');
    normalized
}

/// Emit all TypeScript client files for a generation model.
pub fn emit_typescript_client(model: &GenerationModel) -> Result<Vec<(String, String)>> {
    let ctx = model.typescript_template_context();

    let mut files: Vec<(String, String)> = vec![
        ("metadata.ts".into(), render_ts("typescript/metadata", &ctx)?),
        ("index.ts".into(), render_ts("typescript/index", &ctx)?),
        ("node.ts".into(), render_ts("typescript/node", &ctx)?),
        ("browser.ts".into(), render_ts("typescript/browser", &ctx)?),
        ("_internal/synthesize.ts".into(), render_ts("typescript/synthesize", &ctx)?),
        ("references.ts".into(), render_ts("typescript/references", &ctx)?),
        ("models.ts".into(), render_ts("typescript/models", &ctx)?),
        ("inputs.ts".into(), render_ts("typescript/inputs", &ctx)?),
        ("operations.ts".into(), render_ts("typescript/operations", &ctx)?),
        ("errors.ts".into(), render_ts("typescript/errors", &ctx)?),
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
