//! Generated TypeScript host boundary: every emitted file may only import the
//! host modules assigned to it by the generation contract.

use std::collections::BTreeSet;

use iris_generator::{GenerationModel, emit_typescript_client};

const BLOG_SCHEMA: &str = r#"
table User {
    @@user_id: uuid,
    user_name: utf8,
    active: bool,
}

table Post {
    @@post_id: uuid,
    author: &User,
    title: utf8,
    published: bool,
}
"#;

/// Host package specifiers each generated file is allowed to import.
///
/// Only the two live host entry points may reach a host package: `node.ts` owns
/// the desktop `@yydb/iris` surface and `browser.ts` owns `@yydb/iris-language`.
/// `cloudflare.ts` is a fail-closed edge entry with no host import until the
/// generated D1 artifact lands. Everything else is shared generated code and
/// must stay host neutral.
const HOST_IMPORT_WHITELIST: &[(&str, &[&str])] = &[
    ("metadata.ts", &[]),
    ("index.ts", &[]),
    ("node.ts", &["@yydb/iris/node", "@yydb/iris/types"]),
    ("browser.ts", &["@yydb/iris-language"]),
    ("cloudflare.ts", &[]),
    ("_internal/contract-types.ts", &[]),
    ("_internal/synthesize.ts", &[]),
    ("references.ts", &[]),
    ("models.ts", &[]),
    ("inputs.ts", &[]),
    ("errors.ts", &[]),
    ("operations.ts", &[]),
];

fn allowed_host_imports(name: &str) -> Option<&'static [&'static str]> {
    HOST_IMPORT_WHITELIST.iter().find(|(file, _)| *file == name).map(|(_, allowed)| *allowed)
}

/// Module specifiers of every static `import` / `export ... from` in a file.
fn module_specifiers(source: &str) -> Vec<String> {
    let mut specs = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        let tail = trimmed.split_once("from \"").map(|(_, tail)| tail).or_else(|| trimmed.strip_prefix("import \""));
        if let Some(tail) = tail
            && let Some(specifier) = tail.split_once('"').map(|(specifier, _)| specifier)
        {
            specs.push(specifier.to_owned());
        }
    }
    specs
}

#[test]
fn generated_typescript_imports_stay_inside_host_whitelist() {
    let model = GenerationModel::from_vos_schema(BLOG_SCHEMA).expect("schema");
    let files = emit_typescript_client(&model).expect("emit");

    let generated: BTreeSet<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
    let declared: BTreeSet<&str> = HOST_IMPORT_WHITELIST.iter().map(|(name, _)| *name).collect();
    assert_eq!(
        generated, declared,
        "generated file set drifted from the host import whitelist. Update `HOST_IMPORT_WHITELIST` with the new file's host boundary"
    );

    for (name, content) in &files {
        let allowed = allowed_host_imports(name).expect("whitelist covers every generated file");
        for specifier in module_specifiers(content) {
            if !specifier.starts_with("@yydb/") {
                continue;
            }
            assert!(allowed.contains(&specifier.as_str()), "`{name}` imports `{specifier}`, which is outside its host whitelist {allowed:?}");
        }
    }
}

#[test]
fn browser_entry_stays_on_the_web_language_package() {
    let model = GenerationModel::from_vos_schema(BLOG_SCHEMA).expect("schema");
    let files = emit_typescript_client(&model).expect("emit");
    let browser = files.iter().find(|(name, _)| name == "browser.ts").map(|(_, content)| content.as_str()).expect("browser.ts");

    for specifier in module_specifiers(browser) {
        assert_ne!(specifier, "@yydb/iris", "browser entry must not import the desktop root");
        assert!(!specifier.starts_with("@yydb/iris/"), "browser entry must not import a desktop subpath, found `{specifier}`");
    }
    assert!(browser.contains("@yydb/iris-language"), "{browser}");
}

#[test]
fn shared_generated_layer_never_reaches_a_desktop_entry() {
    let model = GenerationModel::from_vos_schema(BLOG_SCHEMA).expect("schema");
    let files = emit_typescript_client(&model).expect("emit");

    for (name, content) in &files {
        if name == "node.ts" {
            continue;
        }
        for specifier in module_specifiers(content) {
            assert_ne!(specifier, "@yydb/iris", "`{name}` is shared generated code and must not import the desktop root");
            assert!(
                !specifier.starts_with("@yydb/iris/"),
                "`{name}` is shared generated code and must not import desktop subpath `{specifier}`"
            );
        }
    }
}

/// Desktop-side contamination tokens that must never appear in `browser.ts`.
///
/// Web-facing entry points must not reference the native desktop executor,
/// WASM assets, or the internal WASM core package — those are wired up
/// inside `@yydb/iris-language`, not re-declared at the generated entry.
const BROWSER_CONTAMINATION_TOKENS: &[&str] = &["@yydb/iris/node", "@yydb/hermes", "@yydb/iris-unknown-wasm32", ".wasm"];

#[test]
fn browser_entry_is_free_of_desktop_and_wasm_contamination() {
    let model = GenerationModel::from_vos_schema(BLOG_SCHEMA).expect("schema");
    let files = emit_typescript_client(&model).expect("emit");
    let browser = files.iter().find(|(name, _)| name == "browser.ts").map(|(_, content)| content.as_str()).expect("browser.ts");

    for token in BROWSER_CONTAMINATION_TOKENS {
        assert!(!browser.contains(token), "browser.ts must never contain `{token}` (desktop/WASM contamination)\n{browser}");
    }

    // browser.ts is allowed to import @yydb/iris-language and that is the
    // only host package that should appear in this file.
    assert!(browser.contains("@yydb/iris-language"), "browser.ts is the @yydb/iris-language entry");
}

/// Web/Hermes contamination tokens that must never appear in `node.ts`.
///
/// Node entry owns the desktop `@yydb/iris` surface; it must not leak any
/// Web package, Hermes RPC package, or WASM asset paths onto the desktop host.
const NODE_CONTAMINATION_TOKENS: &[&str] = &["@yydb/iris-language", "@yydb/hermes", "@yydb/iris-unknown-wasm32", ".wasm"];

#[test]
fn node_entry_is_free_of_web_and_wasm_contamination() {
    let model = GenerationModel::from_vos_schema(BLOG_SCHEMA).expect("schema");
    let files = emit_typescript_client(&model).expect("emit");
    let node = files.iter().find(|(name, _)| name == "node.ts").map(|(_, content)| content.as_str()).expect("node.ts");

    for token in NODE_CONTAMINATION_TOKENS {
        assert!(!node.contains(token), "node.ts must never contain `{token}` (Web/Hermes/WASM contamination)\n{node}");
    }

    // node.ts is the desktop entry; it imports @yydb/iris/node (executor) and
    // @yydb/iris/types (contract shapes).
    assert!(node.contains("@yydb/iris/node"), "node.ts is the @yydb/iris/node entry");
    assert!(node.contains("@yydb/iris/types"), "node.ts re-exports @yydb/iris/types");
}
