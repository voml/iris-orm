//! Host-neutral runtime drift guard: the committed runtime modules in both
//! TypeScript host packages must equal the `iris-generator` template output.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use iris_generator::emit_typescript_runtime;

/// Runtime modules each package owns on top of the generated shared set.
const HOST_OWNED: &[(&str, &[&str])] = &[
    (
        "projects/iris.ts/packages/iris/src/runtime",
        &[
            "binding-executor.ts",
            "build-operation-request.ts",
            "build-runtime.ts",
            "host.ts",
            "index.ts",
            "negotiate-capabilities.ts",
            "open-binding-session.ts",
        ],
    ),
    (
        "projects/iris.ts/packages/iris-language/src/runtime",
        &["build-runtime.ts", "host.ts", "index.ts", "negotiate-capabilities.ts", "open-binding-session.ts"],
    ),
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../..")
}

fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n")
}

#[test]
fn committed_runtime_modules_match_generator_output() {
    let files = emit_typescript_runtime().expect("render host-neutral runtime modules");
    assert_eq!(files.len(), 8, "unexpected shared runtime module count");

    for (package, _) in HOST_OWNED {
        let runtime_dir = repo_root().join(package);
        for (name, expected) in &files {
            let path = runtime_dir.join(name);
            let actual = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            assert_eq!(
                normalize_newlines(&actual),
                *expected,
                "`{}` drifted from the generator templates, run `cargo run -p iris-generator --bin iris-runtime-sync`",
                path.display(),
            );
        }
    }
}

#[test]
fn runtime_directories_only_hold_generated_or_registered_modules() {
    let files = emit_typescript_runtime().expect("render host-neutral runtime modules");
    let shared: BTreeSet<&str> = files.iter().map(|(name, _)| name.as_str()).collect();

    for (package, host_owned) in HOST_OWNED {
        let runtime_dir = repo_root().join(package);
        let mut actual = BTreeSet::new();
        for entry in std::fs::read_dir(&runtime_dir).unwrap_or_else(|error| panic!("read {}: {error}", runtime_dir.display())) {
            let entry = entry.expect("runtime dir entry");
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if file_name.ends_with(".ts") {
                actual.insert(file_name);
            }
        }
        let expected: BTreeSet<String> =
            shared.iter().map(|name| (*name).to_owned()).chain(host_owned.iter().map(|name| (*name).to_owned())).collect();
        assert_eq!(
            actual,
            expected,
            "`{}` has unregistered modules, move shared code into the templates and list host-specific files in HOST_OWNED",
            runtime_dir.display(),
        );
    }
}
