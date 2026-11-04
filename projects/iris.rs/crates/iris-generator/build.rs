//! Prefers AOT: parse each `templates/**/*.dejavu` to IR JSON under OUT_DIR.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    // Prefer repo-level templates/; fall back to crate-local templates/.
    let repo_templates = manifest_dir.join("../../templates");
    let templates_dir = if repo_templates.is_dir() { repo_templates } else { manifest_dir.join("templates") };
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let aot_dir = out_dir.join("aot");
    fs::create_dir_all(&aot_dir).expect("create OUT_DIR/aot");

    println!("cargo:rerun-if-changed={}", templates_dir.display());
    println!("cargo:rerun-if-changed=build.rs");

    let aot = env::var("CARGO_FEATURE_AOT").is_ok();
    if !aot {
        panic!("iris-generator: enable feature `aot` (default)");
    }

    let mut entries = Vec::new();
    for family in TEMPLATE_FAMILIES {
        if !family_enabled(family) {
            continue;
        }
        let family_dir = templates_dir.join(family);
        if !family_dir.is_dir() {
            panic!("iris-generator: feature `templates-{family}` is enabled but {} is missing", family_dir.display());
        }
        println!("cargo:rerun-if-changed={}", family_dir.display());
        collect_dejavu(&family_dir, &mut entries);
    }
    if entries.is_empty() {
        panic!("iris-generator: enable at least one `templates-*` family");
    }
    entries.sort();

    let mut match_arms = String::new();
    let mut name_list = Vec::new();

    for path in &entries {
        let stem = template_stem(path, &templates_dir);
        let ir_file = stem.replace('/', "__");
        name_list.push(stem.clone());
        println!("cargo:rerun-if-changed={}", path.display());
        let source = fs::read_to_string(path).unwrap_or_else(|e| {
            panic!("read {}: {e}", path.display());
        });
        let doc = dejavu::Dejavu::parse(&source).unwrap_or_else(|e| {
            panic!("AOT parse {}: {e:?}", path.display());
        });
        let ir_path = aot_dir.join(format!("{ir_file}.ir.json"));
        // Compact JSON: this blob is embedded into every consumer (including
        // `iris-wasm`), and `render_aot_ir` only deserializes it, so the
        // pretty-printed form sized the browser artifact without buying anything.
        let json = serde_json::to_string(&doc).expect("serialize IR");
        fs::write(&ir_path, json).expect("write IR JSON");
        match_arms.push_str(&format!(
            "        {stem:?} => render_aot_ir(include_str!(concat!(env!(\"OUT_DIR\"), \"/aot/{ir_file}.ir.json\")), ctx),\n"
        ));
    }

    let registry = format!(
        r#"/// Template names registered from `templates/`.
pub const TEMPLATE_NAMES: &[&str] = &{names:?};

pub(crate) fn render_registered(name: &str, ctx: &serde_json::Value) -> crate::Result<String> {{
    match name {{
{arms}        other => Err(crate::Error::UnknownTemplate(other.to_owned())),
    }}
}}
"#,
        names = name_list,
        arms = match_arms,
    );
    fs::write(out_dir.join("aot_registry.rs"), registry).expect("write aot_registry.rs");
}

/// Template families selected by the `templates-<family>` features. `iris-wasm`
/// turns off the families it cannot render so the browser artifact does not
/// carry their IR payload.
const TEMPLATE_FAMILIES: [&str; 3] = ["typescript", "python", "rust"];

fn family_enabled(family: &str) -> bool {
    let key = format!("CARGO_FEATURE_TEMPLATES_{}", family.to_ascii_uppercase());
    env::var_os(key).is_some()
}

fn collect_dejavu(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir)
    else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_dejavu(&path, out);
        }
        else if path.extension().and_then(|s| s.to_str()).is_some_and(|ext| ext == "dejavu") {
            out.push(path);
        }
    }
}

fn template_stem(path: &Path, templates_dir: &Path) -> String {
    let relative = path.strip_prefix(templates_dir).expect("template path must live under templates/");
    let file = relative.to_string_lossy();
    let without = file.strip_suffix(".dejavu").unwrap_or(&file);
    let without = without.trim_end_matches(".rs").trim_end_matches(".ts");
    without.replace('\\', "/")
}
