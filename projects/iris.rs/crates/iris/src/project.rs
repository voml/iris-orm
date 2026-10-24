//! Project loading and schema I/O (shared by CLI + embedders).

use std::path::{Path, PathBuf};

use crate::{DatasourceConfig, IrisProject, PROJECT_FILE, expand_env};

/// Runtime VON materialized from `iris.config.ts` (not an authoring surface).
pub const RUNTIME_PROJECT_FILE: &str = ".iris/project.von";

/// Resolve the Iris project root from a config file path.
pub fn resolve_project_root(config: &Path) -> PathBuf {
    if is_runtime_project_config(config) {
        return config.parent().and_then(|p| p.parent()).filter(|d| !d.as_os_str().is_empty()).unwrap_or_else(|| Path::new(".")).to_path_buf();
    }
    config.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or_else(|| Path::new(".")).to_path_buf()
}

fn is_runtime_project_config(config: &Path) -> bool {
    config.file_name() == Some(std::ffi::OsStr::new("project.von"))
        && config.parent().is_some_and(|parent| parent.file_name() == Some(std::ffi::OsStr::new(".iris")))
}

/// Load project config (`iris.von` or materialized `.iris/project.von`) and return `(project_dir, project)`.
pub fn load_project(config: &Path) -> Result<(PathBuf, IrisProject), String> {
    let project = IrisProject::load(config).map_err(|e| e.to_string())?;
    Ok((resolve_project_root(config), project))
}

/// Load project from optional config path (defaults to `./iris.von` when present).
pub fn load_project_optional(config: Option<&Path>) -> Result<(Option<PathBuf>, Option<IrisProject>), String> {
    let path = match config {
        Some(p) => Some(p.to_path_buf()),
        None => discover_project_config(Path::new(".")),
    };
    match path {
        None => Ok((None, None)),
        Some(p) => {
            let (dir, project) = load_project(&p)?;
            Ok((Some(dir), Some(project)))
        }
    }
}

/// Load project; error when `iris.von` is missing.
pub fn load_project_required(config: Option<&Path>) -> Result<(PathBuf, IrisProject), String> {
    let (dir, project) = load_project_optional(config)?;
    match (dir, project) {
        (Some(d), Some(p)) => Ok((d, p)),
        _ => Err(format!("iris project config not found; pass --config or create iris.config.ts / ./{PROJECT_FILE}")),
    }
}

/// Discover legacy `iris.von` or materialized `.iris/project.von` under `dir`.
pub fn discover_project_config(dir: &Path) -> Option<PathBuf> {
    let legacy = dir.join(PROJECT_FILE);
    if legacy.is_file() {
        return Some(legacy);
    }
    let runtime = dir.join(RUNTIME_PROJECT_FILE);
    if runtime.is_file() {
        return Some(runtime);
    }
    None
}

/// Materialize a JSON project document to `.iris/project.von` under `project_dir`.
pub fn materialize_runtime_project(project_dir: &Path, document_json: &str) -> Result<PathBuf, String> {
    let project = IrisProject::from_json(document_json).map_err(|e| e.to_string())?;
    let path = project_dir.join(RUNTIME_PROJECT_FILE);
    project.save(&path).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Read merged VOS schema text from project config (`schema` field points at data files; glob ok).
pub fn read_schema(project_dir: &Path, project: &IrisProject) -> Result<String, String> {
    crate::read_schema(project_dir, &project.schema)
}

/// Expand `$MYSQL_URL` / env placeholders in a datasource endpoint.
pub fn expand_endpoint(ds: &DatasourceConfig, source: &str) -> Result<String, String> {
    let template = ds.url.as_ref().or(ds.path.as_ref()).ok_or_else(|| format!("datasource `{source}` missing url/path"))?;
    expand_env(template).map_err(|e| e.to_string())
}

/// Write a file, creating parent directories when needed.
pub fn write_file(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, text).map_err(|e| e.to_string())
}
