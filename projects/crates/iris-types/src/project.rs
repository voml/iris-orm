//! `iris.von` project configuration (VON document, no secrets).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Filename for Iris project configuration.
pub const PROJECT_FILE: &str = "iris.von";

/// Filename for generation/contract lock.
pub const LOCK_FILE: &str = "iris.lock";

/// Stable discriminator for Iris project documents.
pub const PROJECT_FORMAT: &str = "iris.project";

/// Stable discriminator for Iris lock documents.
pub const LOCK_FORMAT: &str = "iris.lock";

/// Default relative schema glob when `schema` is omitted from `iris.von`.
pub const DEFAULT_SCHEMA: &str = "schemas/**/*.iris";

/// Workspace-local Iris tool cache (generate, migrations, lock).
pub const DEFAULT_CACHE_ROOT: &str = ".cache/iris";

/// Default migration plans directory (under [`DEFAULT_CACHE_ROOT`]).
pub const DEFAULT_MIGRATIONS_DIR: &str = ".cache/iris/migrations";

/// Default generate output directory (under [`DEFAULT_CACHE_ROOT`]).
pub const DEFAULT_GENERATE_DIR: &str = ".cache/iris/generate";

/// Default lock file path (under [`DEFAULT_CACHE_ROOT`]).
pub const DEFAULT_LOCK_PATH: &str = ".cache/iris/iris.lock";

fn default_schema() -> String {
    DEFAULT_SCHEMA.into()
}

fn is_default_schema(schema: &str) -> bool {
    schema == DEFAULT_SCHEMA
}

/// How Iris treats schema truth for a datasource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TruthMode {
    /// Native Pull (YYDB / YYDS).
    NativePull,
    /// Managed Push (external DB projected from `.iris` schema).
    ManagedPush,
    /// Adopt Existing (mapping over an observed catalog).
    AdoptExisting,
}

/// Backend kind declared in `iris.von` (no connection secrets here).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatasourceKind {
    /// In-memory reference oracle.
    Reference,
    /// Native YYDB.
    Yydb,
    /// Native YYDS (may be gated).
    Yyds,
    /// SQLite foreign adapter.
    Sqlite,
    /// PostgreSQL foreign adapter.
    Postgres,
    /// MySQL foreign adapter.
    Mysql,
    /// Redis keyspace adapter.
    Redis,
}

/// One named datasource binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasourceConfig {
    /// Backend kind.
    pub kind: DatasourceKind,
    /// Truth mode.
    pub mode: TruthMode,
    /// Filesystem path or URL template. May reference `$ENV_VAR` segments.
    /// Must not embed passwords; use env expansion for secrets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Optional URL template (postgres/mysql/redis), env-expandable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// Optional generate defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerateConfig {
    /// Output directory (default [`DEFAULT_GENERATE_DIR`], workspace-local).
    #[serde(
        default = "default_generate_out",
        skip_serializing_if = "is_default_generate_out"
    )]
    pub out: String,
    /// Target language.
    #[serde(
        default = "default_generate_target",
        skip_serializing_if = "is_default_generate_target"
    )]
    pub target: String,
}

fn default_generate_out() -> String {
    DEFAULT_GENERATE_DIR.into()
}

fn is_default_generate_out(out: &str) -> bool {
    out == DEFAULT_GENERATE_DIR
}

fn default_generate_target() -> String {
    "rust".into()
}

fn is_default_generate_target(target: &str) -> bool {
    target == "rust"
}

impl GenerateConfig {
    /// True when output/target match workspace cache defaults.
    pub fn is_default(&self) -> bool {
        is_default_generate_out(&self.out) && is_default_generate_target(&self.target)
    }
}

impl Default for GenerateConfig {
    fn default() -> Self {
        Self {
            out: default_generate_out(),
            target: default_generate_target(),
        }
    }
}

/// `iris.von` project document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IrisProject {
    /// Discriminator (`iris.project`).
    pub format: String,
    /// Config contract version.
    pub version: i64,
    /// Relative schema path, directory, or glob. Defaults to [`DEFAULT_SCHEMA`].
    #[serde(default = "default_schema", skip_serializing_if = "is_default_schema")]
    pub schema: String,
    /// Named datasources.
    #[serde(default)]
    pub datasources: BTreeMap<String, DatasourceConfig>,
    /// Generate defaults.
    #[serde(default, skip_serializing_if = "GenerateConfig::is_default")]
    pub generate: GenerateConfig,
    /// Optional named topology paths relative to the project (`topologies/*.von`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub topologies: BTreeMap<String, String>,
}

impl Default for IrisProject {
    fn default() -> Self {
        Self {
            format: PROJECT_FORMAT.into(),
            version: 1,
            schema: default_schema(),
            datasources: BTreeMap::new(),
            generate: GenerateConfig::default(),
            topologies: BTreeMap::new(),
        }
    }
}

impl IrisProject {
    /// Minimal valid project using the default schema glob ([`DEFAULT_SCHEMA`]).
    pub fn new() -> Self {
        Self::default()
    }

    /// Project with an explicit schema path (single file, directory, or glob).
    pub fn with_schema(schema: impl Into<String>) -> Self {
        let mut project = Self::new();
        project.schema = schema.into();
        project
    }

    /// Validate discriminator / version / required fields.
    pub fn validate(&self) -> Result<(), ProjectError> {
        if self.format != PROJECT_FORMAT {
            return Err(ProjectError::UnsupportedFormat(self.format.clone()));
        }
        if self.version != 1 {
            return Err(ProjectError::UnsupportedVersion(self.version));
        }
        if self.schema.trim().is_empty() {
            return Err(ProjectError::Invalid(
                "schema path must not be empty".into(),
            ));
        }
        for (name, ds) in &self.datasources {
            if name.trim().is_empty() {
                return Err(ProjectError::Invalid(
                    "datasource name must not be empty".into(),
                ));
            }
            if ds.path.is_none() && ds.url.is_none() && ds.kind != DatasourceKind::Reference {
                return Err(ProjectError::Invalid(format!(
                    "datasource `{name}` needs path or url (except reference)"
                )));
            }
            if let Some(p) = &ds.path {
                reject_inline_secret(p)?;
            }
            if let Some(u) = &ds.url {
                reject_inline_secret(u)?;
            }
        }
        Ok(())
    }

    /// Load and validate from a VON file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ProjectError> {
        let text = std::fs::read_to_string(path.as_ref()).map_err(ProjectError::Io)?;
        Self::parse(&text)
    }

    /// Parse VON text.
    pub fn parse(text: &str) -> Result<Self, ProjectError> {
        let project: Self = von::from_str(text).map_err(ProjectError::Von)?;
        project.validate()?;
        Ok(project)
    }

    /// Canonical VON text.
    pub fn to_von(&self) -> Result<String, ProjectError> {
        self.validate()?;
        von::to_string_indented(self).map_err(ProjectError::Von)
    }

    /// Write canonical VON to disk.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), ProjectError> {
        let text = self.to_von()?;
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).map_err(ProjectError::Io)?;
        }
        std::fs::write(path, text).map_err(ProjectError::Io)
    }

    /// Resolve a datasource by name.
    pub fn datasource(&self, name: &str) -> Result<&DatasourceConfig, ProjectError> {
        self.datasources
            .get(name)
            .ok_or_else(|| ProjectError::UnknownDatasource(name.into()))
    }

    /// Resolve a topology path by name (relative path stored in `topologies`).
    pub fn topology_path(&self, name: &str) -> Result<&str, ProjectError> {
        self.topologies
            .get(name)
            .map(String::as_str)
            .ok_or_else(|| ProjectError::Invalid(format!("unknown topology `{name}`")))
    }
}

/// Generation lock file (`iris.lock`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IrisLock {
    /// Discriminator.
    pub format: String,
    /// Lock version.
    pub version: i64,
    /// Schema fingerprint.
    pub schema_fingerprint: String,
    /// Generator version.
    pub generator_version: String,
    /// Generate target.
    pub target: String,
}

impl IrisLock {
    /// Build a lock record.
    pub fn new(
        schema_fingerprint: impl Into<String>,
        generator_version: impl Into<String>,
        target: impl Into<String>,
    ) -> Self {
        Self {
            format: LOCK_FORMAT.into(),
            version: 1,
            schema_fingerprint: schema_fingerprint.into(),
            generator_version: generator_version.into(),
            target: target.into(),
        }
    }

    /// Canonical VON.
    pub fn to_von(&self) -> Result<String, ProjectError> {
        von::to_string_indented(self).map_err(ProjectError::Von)
    }

    /// Parse lock file.
    pub fn parse(text: &str) -> Result<Self, ProjectError> {
        let lock: Self = von::from_str(text).map_err(ProjectError::Von)?;
        if lock.format != LOCK_FORMAT {
            return Err(ProjectError::UnsupportedFormat(lock.format));
        }
        Ok(lock)
    }
}

/// Expand `$VAR` / `${VAR}` segments using process environment.
pub fn expand_env(template: &str) -> Result<String, ProjectError> {
    let mut out = String::new();
    let chars: Vec<char> = template.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '$' {
            if i + 1 < chars.len() && chars[i + 1] == '{' {
                let end = chars[i + 2..]
                    .iter()
                    .position(|c| *c == '}')
                    .ok_or_else(|| ProjectError::Invalid("unclosed ${} in path/url".into()))?
                    + i
                    + 2;
                let key: String = chars[i + 2..end].iter().collect();
                let val = std::env::var(&key).map_err(|_| ProjectError::MissingEnv(key.clone()))?;
                out.push_str(&val);
                i = end + 1;
                continue;
            }
            let start = i + 1;
            let mut end = start;
            while end < chars.len() && (chars[end].is_ascii_alphanumeric() || chars[end] == '_') {
                end += 1;
            }
            if end == start {
                out.push('$');
                i += 1;
                continue;
            }
            let key: String = chars[start..end].iter().collect();
            let val = std::env::var(&key).map_err(|_| ProjectError::MissingEnv(key.clone()))?;
            out.push_str(&val);
            i = end;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    Ok(out)
}

/// Walk upward from `start` to find a Cargo/pnpm workspace root.
pub fn find_workspace_root(start: &Path) -> PathBuf {
    let mut dir = start.to_path_buf();
    loop {
        if workspace_marker(&dir) {
            return dir;
        }
        if !dir.pop() {
            return start.to_path_buf();
        }
    }
}

fn workspace_marker(dir: &Path) -> bool {
    if dir.join("pnpm-workspace.yaml").is_file() {
        return true;
    }
    let cargo = dir.join("Cargo.toml");
    if cargo.is_file()
        && let Ok(text) = std::fs::read_to_string(cargo)
    {
        return text.contains("[workspace]");
    }
    false
}

fn is_cache_relative(relative: &str) -> bool {
    relative.starts_with(".cache/")
}

/// Resolve a path declared in `iris.von`.
///
/// Paths under `.cache/` anchor at the nearest workspace root (see [`find_workspace_root`]);
/// other relative paths stay project-local.
pub fn resolve_path(project_dir: &Path, relative: &str) -> PathBuf {
    let p = PathBuf::from(relative);
    if p.is_absolute() {
        return p;
    }
    if is_cache_relative(relative) {
        return find_workspace_root(project_dir).join(relative);
    }
    project_dir.join(p)
}

/// Default migration plan path for a datasource (`{source}-plan.von`).
pub fn default_migration_plan(project_dir: &Path, source: &str) -> PathBuf {
    resolve_path(
        project_dir,
        &format!("{DEFAULT_MIGRATIONS_DIR}/{source}-plan.von"),
    )
}

fn reject_inline_secret(value: &str) -> Result<(), ProjectError> {
    let lower = value.to_ascii_lowercase();
    for banned in ["password=", "pwd=", "secret=", "://", ":@"] {
        // Allow redis:// / mysql:// / postgres:// scheme only when no user:pass@ form.
        if banned == "://" {
            continue;
        }
        if lower.contains(banned) && !value.contains('$') {
            return Err(ProjectError::SecretInConfig(format!(
                "refusing inline credential material matching `{banned}`; use $ENV expansion"
            )));
        }
    }
    // user:pass@host pattern without env vars
    if value.contains('@')
        && value.contains(':')
        && !value.contains('$')
        && let Some(scheme_end) = value.find("://")
    {
        let rest = &value[scheme_end + 3..];
        if rest.contains('@') && rest.split('@').next().is_some_and(|u| u.contains(':')) {
            return Err(ProjectError::SecretInConfig(
                "refusing user:password@url in iris.von; use $ENV for secrets".into(),
            ));
        }
    }
    Ok(())
}

/// Project config errors.
#[derive(Debug)]
pub enum ProjectError {
    /// I/O.
    Io(std::io::Error),
    /// VON parse/serialize.
    Von(von::VonError),
    /// Unknown format discriminator.
    UnsupportedFormat(String),
    /// Unsupported version.
    UnsupportedVersion(i64),
    /// Invalid field.
    Invalid(String),
    /// Datasource missing.
    UnknownDatasource(String),
    /// Required environment variable missing.
    MissingEnv(String),
    /// Secrets must not live in iris.von.
    SecretInConfig(String),
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Von(e) => write!(f, "{e}"),
            Self::UnsupportedFormat(s) => write!(f, "unsupported iris.von format `{s}`"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported iris.von version {v}"),
            Self::Invalid(s) | Self::SecretInConfig(s) => write!(f, "{s}"),
            Self::UnknownDatasource(s) => write!(f, "unknown datasource `{s}`"),
            Self::MissingEnv(s) => write!(f, "missing environment variable `{s}`"),
        }
    }
}

impl std::error::Error for ProjectError {}
