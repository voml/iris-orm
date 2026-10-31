//! TypeScript generated-client emitter (Dejavu templates only).

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::{GenerationModel, Result, render, unescape_dejavu_template};

fn render_ts(name: &str, ctx: &Value) -> Result<String> {
    let rendered = unescape_dejavu_template(&render(name, ctx)?);
    Ok(normalize_ts_blank_lines(&rendered))
}

/// Runtime templates are read as ordinary package source, so keep their blank lines.
fn render_runtime_ts(name: &str) -> Result<String> {
    let mut normalized = unescape_dejavu_template(&render(name, &json!({}))?).replace("\r\n", "\n").trim().to_string();
    normalized.push('\n');
    Ok(normalized)
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
    let ctx = model.typescript_template_context()?;

    let mut files: Vec<(String, String)> = vec![
        ("metadata.ts".into(), render_ts("typescript/metadata", &ctx)?),
        ("index.ts".into(), render_ts("typescript/index", &ctx)?),
        ("node.ts".into(), render_ts("typescript/node", &ctx)?),
        ("browser.ts".into(), render_ts("typescript/browser", &ctx)?),
        ("cloudflare.ts".into(), render_ts("typescript/cloudflare", &ctx)?),
        ("_internal/contract-types.ts".into(), render_ts("typescript/contract-types", &ctx)?),
        ("_internal/synthesize.ts".into(), render_ts("typescript/synthesize", &ctx)?),
        ("references.ts".into(), render_ts("typescript/references", &ctx)?),
        ("models.ts".into(), render_ts("typescript/models", &ctx)?),
        ("inputs.ts".into(), render_ts("typescript/inputs", &ctx)?),
        ("errors.ts".into(), render_ts("typescript/errors", &ctx)?),
    ];

    // operations.ts: template base + stubs for projected operations
    let base_ops = render_ts("typescript/operations", &ctx)?;
    let ops_content = if model.operations.is_empty() {
        base_ops
    }
    else {
        let stubs = crate::emit_ts_operation_stubs(model)?;
        let mut merged = base_ops;
        merged.push_str(&stubs);
        normalize_ts_blank_lines(&merged)
    };
    files.push(("operations.ts".into(), ops_content));

    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}

/// Write TypeScript client files into `generate_root` (`iris.config.ts` `generate.out`).
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

/// Host-neutral runtime module stems, rendered from `templates/typescript/runtime/`.
const RUNTIME_TEMPLATE_NAMES: &[&str] =
    &["db-binding", "hex", "operation-binding", "operation-parameters", "parse", "validate-contract", "value-carrier", "wire-row-map"];

/// Emit the host-neutral runtime modules shared by `@yydb/iris` and `@yydb/iris-language`.
///
/// Both host packages keep their own host-specific modules (session construction,
/// capability negotiation, package exports) plus a small `host.ts` type adapter.
/// Everything returned here comes from the one template set under
/// `templates/typescript/runtime/`, so the two committed copies stay identical.
pub fn emit_typescript_runtime() -> Result<Vec<(String, String)>> {
    let mut files: Vec<(String, String)> = Vec::with_capacity(RUNTIME_TEMPLATE_NAMES.len());
    for name in RUNTIME_TEMPLATE_NAMES {
        files.push((format!("{name}.ts"), render_runtime_ts(&format!("typescript/runtime/{name}"))?));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}

/// Write the host-neutral runtime modules into a package `src/runtime/` directory.
pub fn write_typescript_runtime(runtime_dir: &Path) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(runtime_dir)?;
    let mut written = Vec::new();
    for (name, content) in emit_typescript_runtime()? {
        let target = runtime_dir.join(&name);
        let tmp = runtime_dir.join(format!("{name}.tmp"));
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
