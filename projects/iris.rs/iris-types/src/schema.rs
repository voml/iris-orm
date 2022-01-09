//! Multi-file VOS schema loading (`schemas/**/*.iris`).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use vos::ast::Document;

use crate::project::resolve_path;

/// Collect schema file paths from a single file, directory tree, or glob pattern.
pub fn collect_schema_paths(project_dir: &Path, schema_pattern: &str) -> Result<Vec<PathBuf>, String> {
    let resolved = resolve_path(project_dir, schema_pattern);
    if schema_pattern.contains('*') || schema_pattern.contains('?') {
        let pattern = resolved.to_string_lossy().replace('\\', "/");
        let mut paths: Vec<PathBuf> = glob::glob(&pattern)
            .map_err(|e| format!("invalid schema glob `{schema_pattern}`: {e}"))?
            .filter_map(|entry| entry.ok())
            .filter(|path| is_schema_file(path))
            .collect();
        paths.sort();
        if paths.is_empty() {
            return Err(format!(
                "no `.iris` files matched `{schema_pattern}`"
            ));
        }
        return Ok(paths);
    }
    if resolved.is_dir() {
        let mut paths = Vec::new();
        collect_schema_dir(&resolved, &mut paths)?;
        paths.sort();
        if paths.is_empty() {
            return Err(format!(
                "schema directory `{}` contains no `.iris` files",
                resolved.display()
            ));
        }
        return Ok(paths);
    }
    if !resolved.is_file() {
        return Err(format!("schema path not found: {}", resolved.display()));
    }
    if !is_schema_file(&resolved) {
        return Err(format!(
            "schema file must use `.iris`: {}",
            resolved.display()
        ));
    }
    Ok(vec![resolved])
}

/// Read and merge all schema sources declared in `iris.von` (`schema` field).
pub fn read_schema(project_dir: &Path, schema_pattern: &str) -> Result<String, String> {
    let _ = load_schema_document(project_dir, schema_pattern)?;
    let paths = collect_schema_paths(project_dir, schema_pattern)?;
    let mut parts = Vec::with_capacity(paths.len());
    for path in paths {
        parts.push(std::fs::read_to_string(&path).map_err(|e| {
            format!("{}: {e}", path.display())
        })?);
    }
    Ok(parts.join("\n\n"))
}

/// Parse, validate, and merge schema files into one logical document.
pub fn load_schema_document(project_dir: &Path, schema_pattern: &str) -> Result<Document, String> {
    let paths = collect_schema_paths(project_dir, schema_pattern)?;
    let mut parts = Vec::with_capacity(paths.len());
    for path in &paths {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        vos::parser::parse_document(&text).map_err(|d| format_schema_diag(path, &d))?;
        parts.push(text);
    }
    let merged = parts.join("\n\n");
    let document =
        vos::parser::parse_document(&merged).map_err(|d| format_schema_diag(Path::new(schema_pattern), &d))?;
    validate_unique_tables(&document)?;
    Ok(document)
}

fn collect_schema_dir(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_schema_dir(&path, out)?;
        } else if is_schema_file(&path) {
            out.push(path);
        }
    }
    Ok(())
}

fn is_schema_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext == "iris")
}

/// Advisory hints when a table name may not match the generated host class name.
///
/// Iris uses the same identifier for table and generated class/type. PascalCase
/// (e.g. `User`, `GiftOrder`) is recommended so Rust/TypeScript class names align
/// with the physical table name — not enforced at load time.
pub fn table_name_class_hints(doc: &Document) -> Vec<String> {
    let mut hints = Vec::new();
    for table in doc.tables() {
        let name = &table.name;
        let pascal = name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_uppercase())
            && name.chars().all(|c| c.is_ascii_alphanumeric())
            && !name.is_empty();
        if !pascal {
            hints.push(format!(
                "table `{name}`: consider PascalCase (e.g. User, GiftOrder) so the table name matches the generated host class"
            ));
        }
    }
    hints
}

/// Parse schema source and return [`table_name_class_hints`].
pub fn table_name_class_hints_from_source(source: &str) -> Result<Vec<String>, String> {
    let doc = vos::parser::parse_document(source).map_err(|d| {
        d.errors
            .first()
            .map(|e| e.message.clone())
            .unwrap_or_else(|| "schema parse error".into())
    })?;
    Ok(table_name_class_hints(&doc))
}

fn validate_unique_tables(doc: &Document) -> Result<(), String> {
    let mut seen = HashSet::new();
    for table in doc.tables() {
        if !seen.insert(table.name.clone()) {
            return Err(format!("duplicate table `{name}`", name = table.name));
        }
    }
    Ok(())
}

fn format_schema_diag(path: &Path, d: &vos::ast::Diagnostics) -> String {
    let msg = d
        .errors
        .first()
        .map(|e| e.message.as_str())
        .unwrap_or("schema parse error");
    format!("{}: {msg}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn glob_merges_schemas_from_multiple_files() {
        let root = std::env::temp_dir().join(format!(
            "iris-schema-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let schemas = root.join("schemas");
        fs::create_dir_all(&schemas).unwrap();
        fs::write(
            schemas.join("a.iris"),
            "table User { @@id: utf8, name: utf8, }\n",
        )
        .unwrap();
        fs::write(
            schemas.join("b.iris"),
            "table GiftOrder { @@id: utf8, total_cents: i64, }\n",
        )
        .unwrap();
        let doc = load_schema_document(&root, "schemas/**/*.iris").unwrap();
        let names: Vec<_> = doc.tables().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["User", "GiftOrder"]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn lowercase_table_names_load_but_emit_class_hints() {
        let root = std::env::temp_dir().join(format!(
            "iris-schema-hint-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("bad.iris"), "table user { @@id: utf8, }\n").unwrap();
        let doc = load_schema_document(&root, "bad.iris").unwrap();
        assert_eq!(doc.tables().count(), 1);
        let hints = table_name_class_hints(&doc);
        assert_eq!(hints.len(), 1);
        assert!(hints[0].contains("PascalCase"), "{}", hints[0]);
        let _ = fs::remove_dir_all(root);
    }
}
