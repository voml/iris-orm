//! Python client generation smoke (same `GenerationModel` as TypeScript).

use std::path::Path;

use iris_generator::{GenerationModel, emit_target_files, write_python_client};

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

const VECTOR_SCHEMA: &str = r#"
table Doc {
    @@id: uuid,
    embedding: vector,
    body: utf8,
}
"#;

fn file_of<'a>(files: &'a [(String, String)], name: &str) -> &'a str {
    files
        .iter()
        .find(|(path, _)| path == name)
        .map(|(_, content)| content.as_str())
        .unwrap_or_else(|| panic!("missing generated file `{name}`"))
}

#[test]
fn python_target_writes_authoring_surface() {
    let model = GenerationModel::from_vos_schema(BLOG_SCHEMA).expect("schema");
    let root = std::env::temp_dir()
        .join(format!("iris-py-gen-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let paths = write_python_client(&model, &root).expect("write");
    assert_eq!(paths.len(), 10, "python client emits one file per module");
    for name in [
        "__init__.py",
        "models.py",
        "inputs.py",
        "operations.py",
        "errors.py",
        "metadata.py",
        "_internal/__init__.py",
        "_internal/synthesize.py",
        "_internal/executor.py",
        "py.typed",
    ] {
        assert!(root.join(name).is_file(), "missing `{name}`");
    }

    let init = std::fs::read_to_string(root.join("__init__.py")).expect("read init");
    assert!(init.contains("from .operations import Client, create_client"), "the only construction entry is `create_client`:\n{init}");
    assert!(!init.contains("Session"), "the session authoring surface must not be re-exported:\n{init}");

    let operations = std::fs::read_to_string(root.join("operations.py")).expect("read operations");
    assert!(operations.contains("def create_client(dsn: str, **options: Any) -> Client"), "construction takes a DSN:\n{operations}");
    assert!(operations.contains("class Tables:"), "entities live under `client.tables`:\n{operations}");
    assert!(operations.contains("class Transaction:"), "transactions stay on the authoring surface:\n{operations}");
    assert!(operations.contains("def find_many("), "entity delegates expose `find_many`:\n{operations}");
    assert!(operations.contains("def find_near("), "entity delegates expose `find_near`:\n{operations}");
    assert!(operations.contains("self.user = UserDelegate(self._executor)"), "the entity accessor is the lowercase entity name:\n{operations}");
    assert!(!operations.contains("self.User = "), "the entity accessor is never the class name:\n{operations}");
    assert!(operations.contains("return [User(row) for row in rows]"), "reads return entity models:\n{operations}");
    for forbidden in ["$query", "$execute", "def query(", "def execute(", "open_mysql", "open_postgres", "managed_push", "def close("] {
        assert!(!operations.contains(forbidden), "`{forbidden}` must not appear in the authoring surface:\n{operations}");
    }

    let models = std::fs::read_to_string(root.join("models.py")).expect("read models");
    assert!(models.contains("class User:"), "row model per entity:\n{models}");
    assert!(models.contains("class Post:"), "row model per entity:\n{models}");
    assert!(models.contains("        if \"user_id\" in row:"), "row models copy the fields the row carries:\n{models}");
    assert!(!models.contains("TypedDict"), "row values are models, not dicts:\n{models}");

    let inputs = std::fs::read_to_string(root.join("inputs.py")).expect("read inputs");
    assert!(inputs.contains("class UserWhereInput(TypedDict, total=False):"), "condition object per entity:\n{inputs}");
    assert!(inputs.contains("class UserCreateInput(TypedDict):"), "create input per entity:\n{inputs}");
    assert!(inputs.contains("class PostCreateInput(TypedDict):"), "reference fields reach the create input:\n{inputs}");
    assert!(inputs.contains("author: UserRefInput"), "a `&User` field types as a reference input:\n{inputs}");

    let synthesize = std::fs::read_to_string(root.join("_internal/synthesize.py")).expect("read synthesize");
    assert!(synthesize.contains("FILTER_OPS: dict[str, dict[str, tuple[str, ...]]] = {"), "filter surface is inlined:\n{synthesize}");
    assert!(synthesize.contains("REFERENCE_TARGETS: dict[str, dict[str, str]] = {"), "reference targets are inlined:\n{synthesize}");
    assert!(synthesize.contains("def build_declared_operation_request("), "one request builder:\n{synthesize}");

    let metadata = std::fs::read_to_string(root.join("metadata.py")).expect("read metadata");
    assert!(metadata.contains(&format!("SCHEMA_FINGERPRINT = \"{}\"", model.schema_fingerprint)), "fingerprint reaches metadata:\n{metadata}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn python_vector_field_keeps_equality_out_and_near_in() {
    let model = GenerationModel::from_vos_schema(VECTOR_SCHEMA).expect("model");
    let files = emit_target_files(&model, "python", None).expect("python target");
    let inputs = file_of(&files, "inputs.py");
    let where_start = inputs.find("class DocWhereInput").expect("DocWhereInput");
    let create_start = inputs.find("class DocCreateInput").expect("DocCreateInput");
    let where_block = &inputs[where_start..create_start];
    assert!(!where_block.contains("embedding"), "a `vector` field stays out of the where surface:\n{where_block}");
    assert!(
        inputs.contains("vector: Sequence[float]") || file_of(&files, "operations.py").contains("vector: Sequence[float]"),
        "the near vector input is a float sequence"
    );
    let synthesize = file_of(&files, "_internal/synthesize.py");
    assert!(synthesize.contains("\"embedding\": None"), "a bare `vector` field inlines a null declared dimension:\n{synthesize}");
    let operations = file_of(&files, "operations.py");
    assert!(operations.contains("def find_near("), "every entity delegate exposes `find_near`:\n{operations}");
}

#[test]
fn python_target_rejects_unknown_target_via_service() {
    let model = GenerationModel::from_vos_schema(BLOG_SCHEMA).expect("schema");
    let error = emit_target_files(&model, "ruby", None).expect_err("unknown targets must be rejected");
    assert!(matches!(error, iris_generator::Error::UnsupportedTarget(ref target) if target == "ruby"), "{error}");
}

#[test]
fn generated_python_compiles() {
    let python = if cfg!(windows) { "python" } else { "python3" };
    if std::process::Command::new(python).arg("--version").output().is_err() {
        eprintln!("skipping: `{python}` is not on PATH");
        return;
    }
    let model = GenerationModel::from_vos_schema(BLOG_SCHEMA).expect("schema");
    let root = std::env::temp_dir()
        .join(format!("iris-py-compile-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let paths = write_python_client(&model, &root).expect("write");
    let sources: Vec<String> =
        paths.iter().filter(|path| path.extension().is_some_and(|ext| ext == "py")).map(|path| path.display().to_string()).collect();
    let status = std::process::Command::new(python).args(["-m", "py_compile"]).args(&sources).status().expect("run py_compile");
    assert!(status.success(), "generated Python must compile");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn python_target_is_relative_to_the_generate_root() {
    let model = GenerationModel::from_vos_schema(BLOG_SCHEMA).expect("schema");
    let dir = std::env::temp_dir()
        .join(format!("iris-py-root-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let files = emit_target_files(&model, "python", None).expect("python target");
    assert!(files.iter().any(|(name, _)| name == "__init__.py"));
    write_python_client(&model, &dir).expect("write");
    assert!(dir.join("__init__.py").is_file(), "`generate_root` is the package directory itself");
    assert!(!dir.join("src/generated/iris").exists(), "the emitter must not add a second root");
    let nested: &Path = &dir;
    assert!(nested.is_dir());
    let _ = std::fs::remove_dir_all(dir);
}
