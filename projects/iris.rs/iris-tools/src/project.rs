//! iris.von project loading and schema I/O (shared by CLI + embedders).

use std::path::{Path, PathBuf};

use iris::{
    DatasourceConfig, IrisProject, PROJECT_FILE, expand_env,
};

/// Load `iris.von` and return `(project_dir, project)`.
pub fn load_project(config: &Path) -> Result<(PathBuf, IrisProject), String> {
    let project = IrisProject::load(config).map_err(|e| e.to_string())?;
    let dir = config
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    Ok((dir, project))
}

/// Load project from optional config path (defaults to `./iris.von` when present).
pub fn load_project_optional(
    config: Option<&Path>,
) -> Result<(Option<PathBuf>, Option<IrisProject>), String> {
    let path = match config {
        Some(p) => Some(p.to_path_buf()),
        None => {
            let default = PathBuf::from(PROJECT_FILE);
            if default.exists() {
                Some(default)
            } else {
                None
            }
        }
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
        _ => Err(format!(
            "iris.von not found; pass --config or create ./{PROJECT_FILE}"
        )),
    }
}

/// Read merged VOS schema text from `iris.von` (`schema` field; glob ok).
pub fn read_schema(project_dir: &Path, project: &IrisProject) -> Result<String, String> {
    iris::read_schema(project_dir, &project.schema)
}

/// Expand `$MYSQL_URL` / env placeholders in a datasource endpoint.
pub fn expand_endpoint(ds: &DatasourceConfig, source: &str) -> Result<String, String> {
    let template = ds
        .url
        .as_ref()
        .or(ds.path.as_ref())
        .ok_or_else(|| format!("datasource `{source}` missing url/path"))?;
    expand_env(template).map_err(|e| e.to_string())
}

/// Write a file, creating parent directories when needed.
pub fn write_file(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, text).map_err(|e| e.to_string())
}
