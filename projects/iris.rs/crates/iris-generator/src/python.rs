//! Python generated-client emitter (Dejavu templates only).
//!
//! The Python target renders the same `GenerationModel` as the TypeScript
//! target: one schema, one generation model, one operation ABI. Nothing here
//! re-derives schema semantics.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::{Error, GenerationModel, Result, render, unescape_dejavu_template};

fn render_py(name: &str, ctx: &Value) -> Result<String> {
    Ok(normalize_py_source(&unescape_dejavu_template(&render(name, ctx)?)))
}

/// Generated Python is ordinary module source: keep blank lines, drop trailing
/// whitespace, and end with exactly one newline.
fn normalize_py_source(text: &str) -> String {
    let normalized: String = text.replace("\r\n", "\n").lines().map(str::trim_end).collect::<Vec<_>>().join("\n");
    let mut trimmed = normalized.trim_start_matches('\n').trim_end().to_string();
    trimmed.push('\n');
    trimmed
}

/// Emit all Python client module files for a generation model.
pub fn emit_python_client(model: &GenerationModel) -> Result<Vec<(String, String)>> {
    let ctx = model.python_template_context()?;

    let mut files: Vec<(String, String)> = vec![
        ("__init__.py".into(), render_py("python/init.py", &ctx)?),
        ("models.py".into(), render_py("python/models.py", &ctx)?),
        ("inputs.py".into(), render_py("python/inputs.py", &ctx)?),
        ("errors.py".into(), render_py("python/errors.py", &ctx)?),
        ("metadata.py".into(), render_py("python/metadata.py", &ctx)?),
        ("_internal/__init__.py".into(), render_py("python/internal_init.py", &ctx)?),
        ("_internal/synthesize.py".into(), render_py("python/synthesize.py", &ctx)?),
        ("_internal/executor.py".into(), render_py("python/executor.py", &ctx)?),
        ("py.typed".into(), render_py("python/py_typed", &ctx)?),
    ];

    let base_ops = render_py("python/operations.py", &ctx)?;
    let ops_content = if model.operations.is_empty() {
        base_ops
    }
    else {
        let stubs = emit_python_operation_stubs(model)?;
        normalize_py_source(&format!("{base_ops}\n\n{stubs}"))
    };
    files.push(("operations.py".into(), ops_content));

    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}

/// Write the Python client into `generate_root` (`src/generated/iris/` by default).
pub fn write_python_client(model: &GenerationModel, out_dir: &Path) -> Result<Vec<PathBuf>> {
    let root = crate::python_target_dir(out_dir);
    std::fs::create_dir_all(root.join("_internal"))?;
    let mut written = Vec::new();
    for (name, content) in emit_python_client(model)? {
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

/// Declared operations as async methods on the generated `GeneratedOperations`.
///
/// Bodies go through the same declared-operation request the TypeScript target
/// builds: an opaque operation id, the contract fingerprint and positional
/// parameters. No VOS text is authored on the Python side.
pub(crate) fn emit_python_operation_stubs(model: &GenerationModel) -> Result<String> {
    if model.operations.is_empty() {
        return Ok(String::new());
    }
    let declared: std::collections::HashSet<String> = model.tables.iter().map(|table| table.name.clone()).collect();
    let mut out = String::new();
    out.push_str("# --- Projected operation stubs (from VOS OperationProjectionArtifact) ---\n\n");
    out.push_str("class GeneratedOperations:\n");
    out.push_str("    \"\"\"Declared operations of this schema, delegating to the host executor.\"\"\"\n\n");
    out.push_str("    def __init__(self, executor: OperationExecutor) -> None:\n");
    out.push_str("        self._executor = executor\n\n");
    for operation in &model.operations {
        let kind = operation_kind_py_doc(operation.kind.as_str())
            .ok_or_else(|| Error::UnsupportedOperationKind { operation: operation.name.clone(), kind: operation.kind.clone() })?;
        if let Some(ret) = operation.return_type.as_ref() {
            crate::resolve_operation_carrier(&operation.name, ret, &declared)?;
        }
        for param in &operation.params {
            crate::resolve_operation_carrier(&operation.name, &param.vos_type, &declared)?;
        }
        let signature = if operation.params.is_empty() {
            String::new()
        }
        else {
            format!(", *, {}", operation.params.iter().map(|p| p.name.clone()).collect::<Vec<_>>().join(", "))
        };
        let values = operation.params.iter().map(|p| p.name.clone()).collect::<Vec<_>>().join(", ");
        let ref_flags = operation.params.iter().map(|p| p.vos_type.starts_with('&').to_string()).collect::<Vec<_>>().join(", ");
        let returns_unit = operation.return_type.is_none();
        let op_id_lit = serde_json::to_string(&operation.operation_id).expect("operation id serializes");
        out.push_str(&format!(
            "    def {name}(self{signature}) -> Any:\n\
             \x20       \"\"\"VOS {kind} operation `{name}` (op_id `{op_id}`).\"\"\"\n\
             \x20       synthesis = synthesize_operation_call({name_lit}, [{values}], [{ref_flags}])\n\
             \x20       request = build_declared_operation_request({op_id_lit}, synthesis)\n\
             {body}\n",
            name = operation.name,
            name_lit = serde_json::to_string(&operation.name).expect("operation name serializes"),
            kind = kind,
            op_id = operation.operation_id,
            signature = signature,
            values = values,
            ref_flags = ref_flags,
            body = if returns_unit {
                "        self._executor.execute_unit(request)\n        return None"
            }
            else {
                "        return self._executor.execute(request)"
            },
        ));
    }
    Ok(out)
}

fn operation_kind_py_doc(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "query" => "query",
        "udf" => "udf",
        "micro" => "micro",
        "macro" => "macro",
        _ => return None,
    })
}
