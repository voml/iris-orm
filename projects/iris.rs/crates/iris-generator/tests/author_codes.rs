//! The author surface may only carry the eight semantic codes owned by
//! `iris-host`. Every other IRIS literal is a diagnostic that must never
//! occupy a `code` position.

use std::{
    fs,
    path::{Path, PathBuf},
};

const AUTHOR_CODES: &[&str] = &[
    "IRIS-UNIQUE-CONFLICT",
    "IRIS-NOT-FOUND",
    "IRIS-VECTOR-DIMENSION",
    "IRIS-SCHEMA-MISMATCH",
    "IRIS-CONNECTION-FAILED",
    "IRIS-INVALID-ARGUMENT",
    "IRIS-TRANSACTION-FAILED",
    "IRIS-INTERNAL",
];

/// Diagnostic literals that are allowed to appear in templates and shipped
/// runtimes. Anything else fails the scan and has to be classified first.
const DIAGNOSTIC_CODES: &[&str] = &[
    "IRIS-SESSION-CLOSED",
    "IRIS-PARAM-ENCODING",
    "IRIS-OPERATION-INVALID",
    "IRIS-EXEC-FAILED",
    "IRIS-TRANSACTION-UNSUPPORTED",
    "IRIS-TRANSACTION-ALREADY-OPEN",
    "IRIS-TRANSACTION-NOT-OPEN",
    "IRIS-MANAGED-PUSH-UNSUPPORTED",
    "IRIS-FILTER-FIELD-UNKNOWN",
    "IRIS-FILTER-OPERATOR-NOT-SUPPORTED",
    "IRIS-VECTOR-INPUT",
    "IRIS-VECTOR-NEAR-FIELD",
    "IRIS-VECTOR-NEAR-INPUT",
    "IRIS-VECTOR-NEAR-METRIC",
    "IRIS-VECTOR-NEAR-VECTOR",
    "IRIS-VECTOR-NEAR-DIMENSION",
    "IRIS-VECTOR-NEAR-THRESHOLD",
    "IRIS-VECTOR-NEAR-ZERO-NORM",
    "IRIS-VECTOR-NEAR-CARRIER",
    "IRIS-ESCAPE-HATCH-REMOVED",
    "IRIS-CONTRACT-MISMATCH",
    "IRIS-UNSUPPORTED-OPERATION",
    "IRIS-VALUE-CARRIER",
    "IRIS-PLAN-REJECTED",
    "IRIS-D1-EDGE-ARTIFACT-PENDING",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(4).expect("repository root").to_path_buf()
}

fn collect(dir: &Path, extensions: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir)
    else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, extensions, out);
        }
        else if path.extension().and_then(|extension| extension.to_str()).is_some_and(|extension| extensions.contains(&extension)) {
            out.push(path);
        }
    }
}

fn iris_literals(source: &str) -> Vec<String> {
    let bytes = source.as_bytes();
    let mut literals = Vec::new();
    let mut index = 0;
    while index + 5 <= bytes.len() {
        if &bytes[index..index + 5] == b"IRIS-" {
            let mut end = index + 5;
            while end < bytes.len() && (bytes[end].is_ascii_uppercase() || bytes[end].is_ascii_digit() || bytes[end] == b'-') {
                end += 1;
            }
            literals.push(source[index..end].to_string());
            index = end;
        }
        else {
            index += 1;
        }
    }
    literals
}

/// Values written into an author `code` position on the same line.
fn assigned_code_values(source: &str) -> Vec<String> {
    let mut values = Vec::new();
    for line in source.lines() {
        for marker in ["code:", "code="] {
            let mut offset = 0;
            while let Some(found) = line[offset..].find(marker) {
                let start = offset + found;
                let boundary = start == 0 || !line.as_bytes()[start - 1].is_ascii_alphanumeric() && line.as_bytes()[start - 1] != b'_';
                let rest = &line[start + marker.len()..];
                if boundary {
                    if let Some(quoted) = rest.find("\"IRIS-") {
                        let value = &rest[quoted + 1..];
                        let end = value.find('"').unwrap_or(value.len());
                        values.push(value[..end].to_string());
                    }
                }
                offset = start + marker.len();
            }
        }
        if let Some(start) = line.find("return [\"IRIS-") {
            let value = &line[start + "return [\"".len()..];
            let end = value.find('"').unwrap_or(value.len());
            values.push(value[..end].to_string());
        }
    }
    values
}

fn scan(files: &[PathBuf], label: &str) -> Vec<String> {
    let mut violations = Vec::new();
    for path in files {
        let Ok(source) = fs::read_to_string(path)
        else {
            continue;
        };
        for literal in iris_literals(&source) {
            if !AUTHOR_CODES.contains(&literal.as_str()) && !DIAGNOSTIC_CODES.contains(&literal.as_str()) {
                violations.push(format!("{label}: `{literal}` in {} is not classified", path.display()));
            }
        }
        for value in assigned_code_values(&source) {
            if !AUTHOR_CODES.contains(&value.as_str()) {
                violations.push(format!("{label}: `{value}` is used as an author code in {}", path.display()));
            }
        }
    }
    violations
}

#[test]
fn author_surface_only_emits_the_eight_codes() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    collect(&manifest.join("templates/typescript"), &["dejavu"], &mut files);
    collect(&manifest.join("templates/python"), &["dejavu"], &mut files);
    let root = repo_root();
    collect(&root.join("projects/iris.ts/packages/iris/src"), &["ts"], &mut files);
    collect(&root.join("projects/iris.ts/packages/iris-language/src"), &["ts"], &mut files);
    collect(&root.join("projects/iris.py/packages/iris/src"), &["py", "pyi"], &mut files);

    let model = iris_generator::GenerationModel::from_vos_schema(
        r#"
table Doc {
    @@doc_id: uuid,
    title: utf8,
    body: utf8?,
    views: i64,
    published: bool,
}

table Chunk {
    @@chunk_id: uuid,
    doc: &Doc,
    embedding: vector<3>,
    label: utf8,
}
"#,
    )
    .expect("schema");
    for (name, content) in iris_generator::emit_typescript_client(&model).expect("emit typescript") {
        assert!(
            !assigned_code_values(&content).iter().any(|value| !AUTHOR_CODES.contains(&value.as_str())),
            "typescript `{name}` emits an author code outside the table"
        );
        for literal in iris_literals(&content) {
            assert!(
                AUTHOR_CODES.contains(&literal.as_str()) || DIAGNOSTIC_CODES.contains(&literal.as_str()),
                "typescript `{name}` emits unclassified `{literal}`"
            );
        }
    }
    for (name, content) in iris_generator::emit_python_client(&model).expect("emit python") {
        assert!(
            !assigned_code_values(&content).iter().any(|value| !AUTHOR_CODES.contains(&value.as_str())),
            "python `{name}` emits an author code outside the table"
        );
        for literal in iris_literals(&content) {
            assert!(
                AUTHOR_CODES.contains(&literal.as_str()) || DIAGNOSTIC_CODES.contains(&literal.as_str()),
                "python `{name}` emits unclassified `{literal}`"
            );
        }
    }

    let violations = scan(&files, "source");
    assert!(violations.is_empty(), "{}", violations.join("\n"));

    let mut observed = std::collections::BTreeSet::new();
    for path in &files {
        if let Ok(source) = fs::read_to_string(path) {
            observed.extend(assigned_code_values(&source));
        }
    }
    for expected in [
        "IRIS-CONNECTION-FAILED",
        "IRIS-INVALID-ARGUMENT",
        "IRIS-INTERNAL",
        "IRIS-NOT-FOUND",
        "IRIS-SCHEMA-MISMATCH",
        "IRIS-TRANSACTION-FAILED",
        "IRIS-VECTOR-DIMENSION",
    ] {
        assert!(observed.contains(expected), "the guard must observe `{expected}` in an author code position");
    }
}
