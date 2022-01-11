//! Iris CLI -- check / generate / doctor / inspect / adopt / migrate.
//!
//! No SQL query surface. Datasource secrets come from environment expansion
//! declared in `iris.von`, never from inline passwords.
//!
//! Live CLI bindings: SQLite / PostgreSQL / MySQL (relational inspect/adopt/migrate)
//! and Redis (connectivity + keyspace adopt draft; no relational migrate).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use iris::{
    AccessKind, CacheWatermarkProbe, CapabilitySet, ComponentRole, ConsistencyIntent,
    DatasourceConfig, DatasourceKind, DriftReport, FsObjectStore, IrisLock, IrisProject, LOCK_FILE,
    LocalProjectionStore, LogicalMigrationPlan, MappingManifest, OBJECT_HASH_ALG_BLAKE3,
    ObjectPolicy, ObservedCatalog, PROJECT_FILE, PhysicalExplain, Planner, ProjectionDocument,
    TOPOLOGY_DIR, TopologyContract, TruthMode, activate_topology, assert_explain_safe, expand_env,
    explain_topology, physical_explain_from_plan, projection_status, projection_status_offline,
    resolve_path, verify_projection, verify_report, DEFAULT_GENERATE_DIR, DEFAULT_LOCK_PATH,
};
use iris_adapter_mysql::MysqlSource;
use iris_adapter_postgres::PostgresSource;
use iris_adapter_redis::{RedisSource, RedisWatermarkProbe};
use iris_adapter_sqlite::SqliteSource;
use iris_generator::GenerationModel;

#[derive(Parser, Debug)]
#[command(
    name = "iris",
    version = iris::version(),
    about = "Iris ORM CLI (VOS data-access)"
)]
struct Cli {
    /// Path to `iris.von` (defaults to ./iris.von when present).
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Print Iris version.
    Version,
    /// Parse and validate a VOS schema (uses iris.von schema when omitted).
    Check {
        /// Optional `.iris` path; defaults to `schema` from iris.von (`schemas/**/*.iris`).
        schema: Option<PathBuf>,
    },
    /// Generate host-native bindings via Dejavu.
    Generate {
        schema: Option<PathBuf>,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, default_value = "rust")]
        target: String,
    },
    /// Print datasource capability summaries.
    Capabilities,
    /// Local diagnostics including iris.von + YYDS readiness.
    Doctor,
    /// Inspect a datasource catalog (or Redis connectivity + mapping).
    Inspect {
        #[arg(long)]
        source: String,
    },
    /// Adopt plan helpers.
    Adopt {
        #[command(subcommand)]
        command: AdoptCmd,
    },
    /// Migration plan/apply/verify helpers (relational adapters only).
    Migrate {
        #[command(subcommand)]
        command: MigrateCmd,
    },
    /// Composite topology helpers (Phase 10-A: offline plan/verify).
    Topology {
        #[command(subcommand)]
        command: TopologyCmd,
    },
    /// Explain composite route / consistency / watermarks / fallback (Phase 10-D).
    Explain {
        #[arg(long)]
        topology: String,
        /// identity_read | filtered_query | search | vector_nearest | write | bytes_range | effect
        #[arg(long)]
        op: String,
        /// authoritative | read_your_writes | eventual | bounded_stale:<secs> | projection_required:<component>
        #[arg(long, default_value = "authoritative")]
        intent: String,
        #[arg(long)]
        table: Option<String>,
        /// Optional VOS source for a physical-plan sketch (no bind values emitted).
        #[arg(long)]
        vos: Option<String>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Projection helpers (Phase 10-D: status for Cache; rebuild/verify later).
    Projection {
        #[command(subcommand)]
        command: ProjectionCmd,
    },
    /// Object store helpers (Phase 10-E: status / GC for local FsObjectStore).
    Object {
        #[command(subcommand)]
        command: ObjectCmd,
    },
}

#[derive(Subcommand, Debug)]
enum AdoptCmd {
    /// Write a reviewable mapping manifest (does not apply).
    Plan {
        #[arg(long)]
        source: String,
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
enum MigrateCmd {
    /// Write a logical migration plan for Managed Push (does not apply).
    Plan {
        #[arg(long)]
        source: String,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Apply a previously written logical plan (requires --yes).
    Apply {
        #[arg(long)]
        source: String,
        #[arg(long)]
        plan: PathBuf,
        #[arg(long)]
        yes: bool,
    },
    /// Re-inspect and report drift against the project schema.
    Verify {
        #[arg(long)]
        source: String,
    },
}

#[derive(Subcommand, Debug)]
enum TopologyCmd {
    /// Validate a topology contract (offline).
    Verify {
        /// Path to `topologies/*.von`, or a name from iris.von `topologies`.
        #[arg(long)]
        topology: String,
    },
    /// Emit an offline CompositePlan for an access kind (does not activate).
    Plan {
        #[arg(long)]
        topology: String,
        /// identity_read | filtered_query | search | vector_nearest | write | bytes_range | effect
        #[arg(long)]
        op: String,
        /// authoritative | read_your_writes | eventual | bounded_stale:<secs> | projection_required:<component>
        #[arg(long, default_value = "authoritative")]
        intent: String,
        #[arg(long)]
        table: Option<String>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Publish a topology version (version handshake / rolling upgrade).
    Activate {
        #[arg(long)]
        topology: String,
        /// Directory for `{id}.active.von` (default: `<project>/topologies`).
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Allow topology_version downgrade (dangerous).
        #[arg(long, default_value_t = false)]
        force: bool,
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
enum ProjectionCmd {
    /// Report Cache projection watermark status (offline or --live).
    Status {
        #[arg(long)]
        topology: String,
        /// Limit to one Cache component id.
        #[arg(long)]
        component: Option<String>,
        /// Probe live Cache watermarks via topology datasource bindings.
        #[arg(long, default_value_t = false)]
        live: bool,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Rebuild a Search/Vector projection into a new generation, then switch alias.
    Rebuild {
        #[arg(long)]
        topology: String,
        /// Topology component id (SearchProjection or VectorProjection).
        #[arg(long)]
        component: String,
        /// Local projection store root (generation + alias files).
        #[arg(long)]
        root: PathBuf,
        /// VON seed of projection documents (authority/outbox replay stand-in).
        #[arg(long)]
        seed: PathBuf,
        /// Schema fingerprint stamped on the new generation.
        #[arg(long, default_value = "")]
        schema_fingerprint: String,
        /// Required confirmation.
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Verify an active Search/Vector generation (or Cache route declaration).
    Verify {
        #[arg(long)]
        topology: String,
        /// Component id (Cache / SearchProjection / VectorProjection).
        #[arg(long)]
        component: String,
        /// Local projection store root (required for Search/Vector).
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
enum ObjectCmd {
    /// Report local object-store lifecycle status.
    Status {
        /// Filesystem object store root.
        #[arg(long)]
        root: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// GC aborted/deleting and orphan pending/verified objects.
    Gc {
        #[arg(long)]
        root: PathBuf,
        /// Required confirmation (destructive).
        #[arg(long)]
        yes: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Commands::Version => {
            println!("iris {}", iris::version());
            Ok(())
        }
        Commands::Check { schema } => {
            let (project_dir, project) = load_project_optional(cli.config.as_deref())?;
            if schema.is_none() {
                let (dir, proj) = match (project_dir, project) {
                    (Some(d), Some(p)) => (d, p),
                    _ => {
                        return Err(
                            "schema path required (or provide iris.von with a schema field)".into(),
                        );
                    }
                };
                return cmd_check_merged(&dir, &proj.schema);
            }
            let schema_path =
                resolve_schema_path(project_dir.as_deref(), project.as_ref(), schema)?;
            let is_dir = schema_path.is_dir();
            let pattern_owned = schema_path.to_string_lossy().into_owned();
            if pattern_owned.contains('*') || pattern_owned.contains('?') || is_dir {
                let dir = if is_dir {
                    schema_path
                } else {
                    project_dir.unwrap_or_else(|| PathBuf::from("."))
                };
                let pat = if is_dir {
                    format!("{}/**/*.iris", dir.display()).replace('\\', "/")
                } else {
                    pattern_owned
                };
                cmd_check_merged(&dir, &pat)
            } else {
                cmd_check(&schema_path)
            }
        }
        Commands::Generate {
            schema,
            out,
            target,
        } => {
            let (project_dir, project) = load_project_optional(cli.config.as_deref())?;
            let out_dir = out.unwrap_or_else(|| {
                project
                    .as_ref()
                    .map(|p| {
                        resolve_path(
                            project_dir.as_deref().unwrap_or(Path::new(".")),
                            &p.generate.out,
                        )
                    })
                    .unwrap_or_else(|| {
                        resolve_path(
                            project_dir.as_deref().unwrap_or(Path::new(".")),
                            DEFAULT_GENERATE_DIR,
                        )
                    })
            });
            if schema.is_none() {
                let (dir, proj) = match (project_dir, project) {
                    (Some(d), Some(p)) => (d, p),
                    _ => {
                        return Err(
                            "schema path required (or provide iris.von with a schema field)".into(),
                        );
                    }
                };
                return cmd_generate_merged(&dir, &proj.schema, &out_dir, &target);
            }
            let schema_path =
                resolve_schema_path(project_dir.as_deref(), project.as_ref(), schema)?;
            let is_dir = schema_path.is_dir();
            let pattern_owned = schema_path.to_string_lossy().into_owned();
            if pattern_owned.contains('*') || pattern_owned.contains('?') || is_dir {
                let dir = if is_dir {
                    schema_path
                } else {
                    project_dir.unwrap_or_else(|| PathBuf::from("."))
                };
                let pat = if is_dir {
                    format!("{}/**/*.iris", dir.display()).replace('\\', "/")
                } else {
                    pattern_owned
                };
                cmd_generate_merged(&dir, &pat, &out_dir, &target)
            } else {
                cmd_generate(&schema_path, &out_dir, &target, project_dir.as_deref())
            }
        }
        Commands::Capabilities => {
            cmd_capabilities();
            Ok(())
        }
        Commands::Doctor => {
            let (project_dir, project) = load_project_optional(cli.config.as_deref())?;
            cmd_doctor(project_dir.as_deref(), project.as_ref());
            Ok(())
        }
        Commands::Inspect { source } => {
            let (project_dir, project) = load_project_required(cli.config.as_deref())?;
            cmd_inspect(&project_dir, &project, &source)
        }
        Commands::Adopt {
            command: AdoptCmd::Plan { source, out },
        } => {
            let (project_dir, project) = load_project_required(cli.config.as_deref())?;
            cmd_adopt_plan(&project_dir, &project, &source, out.as_deref())
        }
        Commands::Migrate { command } => {
            let config = cli
                .config
                .clone()
                .unwrap_or_else(|| PathBuf::from(PROJECT_FILE));
            match command {
                MigrateCmd::Plan { source, out } => {
                    cmd_migrate_plan(&config, &source, out.as_deref())
                }
                MigrateCmd::Apply { source, plan, yes } => {
                    cmd_migrate_apply(&config, &source, &plan, yes)
                }
                MigrateCmd::Verify { source } => cmd_migrate_verify(&config, &source),
            }
        }
        Commands::Topology { command } => {
            let (project_dir, project) = load_project_optional(cli.config.as_deref())?;
            match command {
                TopologyCmd::Verify { topology } => {
                    cmd_topology_verify(project_dir.as_deref(), project.as_ref(), &topology)
                }
                TopologyCmd::Plan {
                    topology,
                    op,
                    intent,
                    table,
                    out,
                } => cmd_topology_plan(
                    project_dir.as_deref(),
                    project.as_ref(),
                    &topology,
                    &op,
                    &intent,
                    table.as_deref(),
                    out.as_deref(),
                ),
                TopologyCmd::Activate {
                    topology,
                    state_dir,
                    force,
                    out,
                } => cmd_topology_activate(
                    project_dir.as_deref(),
                    project.as_ref(),
                    &topology,
                    state_dir.as_deref(),
                    force,
                    out.as_deref(),
                ),
            }
        }
        Commands::Explain {
            topology,
            op,
            intent,
            table,
            vos,
            out,
        } => {
            let (project_dir, project) = load_project_optional(cli.config.as_deref())?;
            cmd_explain(
                project_dir.as_deref(),
                project.as_ref(),
                &topology,
                &op,
                &intent,
                table.as_deref(),
                vos.as_deref(),
                out.as_deref(),
            )
        }
        Commands::Projection { command } => {
            let (project_dir, project) = load_project_optional(cli.config.as_deref())?;
            match command {
                ProjectionCmd::Status {
                    topology,
                    component,
                    live,
                    out,
                } => cmd_projection_status(
                    project_dir.as_deref(),
                    project.as_ref(),
                    &topology,
                    component.as_deref(),
                    live,
                    out.as_deref(),
                ),
                ProjectionCmd::Rebuild {
                    topology,
                    component,
                    root,
                    seed,
                    schema_fingerprint,
                    yes,
                    out,
                } => cmd_projection_rebuild(
                    project_dir.as_deref(),
                    project.as_ref(),
                    &topology,
                    &component,
                    &root,
                    &seed,
                    &schema_fingerprint,
                    yes,
                    out.as_deref(),
                ),
                ProjectionCmd::Verify {
                    topology,
                    component,
                    root,
                    out,
                } => cmd_projection_verify(
                    project_dir.as_deref(),
                    project.as_ref(),
                    &topology,
                    &component,
                    root.as_deref(),
                    out.as_deref(),
                ),
            }
        }
        Commands::Object { command } => match command {
            ObjectCmd::Status { root, out } => cmd_object_status(&root, out.as_deref()),
            ObjectCmd::Gc { root, yes } => cmd_object_gc(&root, yes),
        },
    }
}

fn load_project_optional(
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
            let project = IrisProject::load(&p).map_err(|e| e.to_string())?;
            let dir = p
                .parent()
                .filter(|d| !d.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf();
            Ok((Some(dir), Some(project)))
        }
    }
}

fn load_project_required(config: Option<&Path>) -> Result<(PathBuf, IrisProject), String> {
    let (dir, project) = load_project_optional(config)?;
    match (dir, project) {
        (Some(d), Some(p)) => Ok((d, p)),
        _ => Err(format!(
            "iris.von not found; pass --config or create ./{PROJECT_FILE}"
        )),
    }
}

fn resolve_schema_path(
    project_dir: Option<&Path>,
    project: Option<&IrisProject>,
    schema: Option<PathBuf>,
) -> Result<PathBuf, String> {
    if let Some(s) = schema {
        return Ok(s);
    }
    let project = project.ok_or_else(|| {
        "schema path required (or provide iris.von with a schema field)".to_string()
    })?;
    let dir = project_dir.unwrap_or(Path::new("."));
    Ok(resolve_path(dir, &project.schema))
}

fn read_schema(project_dir: &Path, project: &IrisProject) -> Result<String, String> {
    iris_tools::read_schema(project_dir, project)
}

fn expand_endpoint(ds: &DatasourceConfig, source: &str) -> Result<String, String> {
    let template = ds
        .url
        .as_ref()
        .or(ds.path.as_ref())
        .ok_or_else(|| format!("datasource `{source}` missing url/path"))?;
    expand_env(template).map_err(|e| e.to_string())
}

fn default_mapping_path(project_dir: &Path, source: &str) -> PathBuf {
    project_dir.join("mappings").join(format!("{source}.von"))
}

fn print_catalog(source: &str, catalog: &ObservedCatalog) {
    println!(
        "inspect source={source} backend={} tables={}",
        catalog.backend_id,
        catalog.tables.len()
    );
    for table in &catalog.tables {
        let pk = table
            .columns
            .iter()
            .filter(|c| c.primary_key)
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "  - {} ({} cols, pk=[{}])",
            table.name,
            table.columns.len(),
            pk
        );
    }
}

fn write_von_file(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, text).map_err(|e| e.to_string())
}

fn write_relational_adopt(out_path: &Path, manifest: &MappingManifest) -> Result<(), String> {
    let text = von::to_string_indented(manifest).map_err(|e| e.to_string())?;
    write_von_file(out_path, &text)?;
    let blockers: usize = manifest.tables.iter().map(|t| t.blockers.len()).sum();
    println!(
        "wrote adopt plan {} (tables={}, blockers={})",
        out_path.display(),
        manifest.tables.len(),
        blockers
    );
    Ok(())
}

fn cmd_check_merged(project_dir: &Path, pattern: &str) -> Result<(), String> {
    let document = iris::load_schema_document(project_dir, pattern)?;
    for hint in iris::table_name_class_hints(&document) {
        eprintln!("hint: {hint}");
    }
    let source = iris::read_schema(project_dir, pattern)?;
    cmd_check_source(&source)
}

fn cmd_check(schema: &Path) -> Result<(), String> {
    let source = std::fs::read_to_string(schema).map_err(|e| e.to_string())?;
    cmd_check_source(&source)
}

fn cmd_check_source(source: &str) -> Result<(), String> {
    for hint in iris::table_name_class_hints_from_source(source)? {
        eprintln!("hint: {hint}");
    }
    let model = GenerationModel::from_vos_schema(&source).map_err(|e| e.to_string())?;
    println!(
        "ok: {} table(s), fingerprint={}, generator={}",
        model.tables.len(),
        model.schema_fingerprint,
        model.generator_version
    );
    for table in &model.tables {
        let pk = table
            .fields
            .iter()
            .filter(|f| f.primary)
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "  - {} ({} fields, pk=[{}])",
            table.name,
            table.fields.len(),
            pk
        );
    }
    Ok(())
}

fn cmd_generate_merged(
    project_dir: &Path,
    pattern: &str,
    out: &Path,
    target: &str,
) -> Result<(), String> {
    let source = iris::read_schema(project_dir, pattern)?;
    cmd_generate_source(&source, out, target, Some(project_dir))
}

fn cmd_generate(
    schema: &Path,
    out: &Path,
    target: &str,
    project_dir: Option<&Path>,
) -> Result<(), String> {
    let source = std::fs::read_to_string(schema).map_err(|e| e.to_string())?;
    cmd_generate_source(&source, out, target, project_dir)
}

fn cmd_generate_source(
    source: &str,
    out: &Path,
    target: &str,
    project_dir: Option<&Path>,
) -> Result<(), String> {
    if target != "rust" {
        return Err(format!(
            "unsupported generate target `{target}` (iris-tools owns multi-language generate; currently only `rust` is shipped)"
        ));
    }
    let model = GenerationModel::from_vos_schema(source).map_err(|e| e.to_string())?;
    let path = iris_generator::write_rust_domain(&model, out).map_err(|e| e.to_string())?;
    let lock = IrisLock::new(&model.schema_fingerprint, &model.generator_version, target);
    let lock_path = resolve_path(
        project_dir.unwrap_or(Path::new(".")),
        DEFAULT_LOCK_PATH,
    );
    write_von_file(&lock_path, &lock.to_von().map_err(|e| e.to_string())?)?;
    println!(
        "generated {} (fingerprint={})",
        path.display(),
        model.schema_fingerprint
    );
    println!("wrote {}", lock_path.display());
    Ok(())
}

fn cmd_capabilities() {
    println!("backend capability summary (Rust reference implementation):");
    println!("  reference  query=full write=none   notes=in-memory oracle");
    println!("  yydb       query=full write=full   notes=native VOS");
    println!("  yyds       query=full write=full   notes=gated until VOS executor");
    println!("  sqlite     query=full write=full   notes=foreign adapter (CLI live)");
    println!("  postgres   query=full write=full   notes=foreign adapter (CLI live)");
    println!("  mysql      query=full write=full   notes=foreign adapter (CLI live)");
    println!("  redis      query=scan-only write=full notes=keyspace PK only (CLI live)");
}

fn cmd_doctor(project_dir: Option<&Path>, project: Option<&IrisProject>) {
    println!("iris {}", iris::version());
    println!("no-sql-invariant: public surfaces must not speak SQL");
    match project {
        Some(p) => {
            println!(
                "project: dir={} schema={} datasources={}",
                project_dir.unwrap_or(Path::new(".")).display(),
                p.schema,
                p.datasources.len()
            );
            for (name, ds) in &p.datasources {
                println!("  - {name}: kind={:?} mode={:?}", ds.kind, ds.mode);
            }
        }
        None => println!("project: (no iris.von in cwd)"),
    }
    println!("cli live: sqlite, postgres, mysql, redis (keyspace)");
    println!(
        "composite: topology plan|verify|activate; explain; projection status|rebuild|verify; object status|gc"
    );
    println!("composite conformance: iris-types tests composite_conformance_15_6 (§15.6)");
    println!(
        "object store: local FsObjectStore pending->verified->committed reference (Phase 10-E)"
    );
    println!(
        "projections: search/vector candidates -> authority hydrate; rebuild = generation+alias"
    );
    let yyds = iris_connector_yyds::YydsSource::readiness();
    println!(
        "yyds readiness: ready={} code={}",
        yyds.is_ready(),
        yyds.code
    );
    if !yyds.is_ready() {
        println!("  {}", yyds.message);
    }
    println!(
        "generator: aot={} templates={:?}",
        iris_generator::prefers_aot(),
        iris_generator::TEMPLATE_NAMES
    );
}

fn cmd_inspect(project_dir: &Path, project: &IrisProject, source: &str) -> Result<(), String> {
    let ds = project.datasource(source).map_err(|e| e.to_string())?;
    match ds.kind {
        DatasourceKind::Sqlite => {
            let path = resolve_path(project_dir, &expand_endpoint(ds, source)?);
            let db = SqliteSource::open(path).map_err(|e| e.to_string())?;
            print_catalog(source, &db.inspect().map_err(|e| e.to_string())?);
        }
        DatasourceKind::Postgres => {
            let url = expand_endpoint(ds, source)?;
            let db = PostgresSource::connect(&url).map_err(|e| e.to_string())?;
            print_catalog(source, &db.inspect().map_err(|e| e.to_string())?);
        }
        DatasourceKind::Mysql => {
            let url = expand_endpoint(ds, source)?;
            let db = MysqlSource::connect(&url).map_err(|e| e.to_string())?;
            print_catalog(source, &db.inspect().map_err(|e| e.to_string())?);
        }
        DatasourceKind::Redis => {
            let url = expand_endpoint(ds, source)?;
            RedisSource::ping_url(&url).map_err(|e| e.to_string())?;
            let mapping_path = default_mapping_path(project_dir, source);
            if mapping_path.exists() {
                let text = std::fs::read_to_string(&mapping_path).map_err(|e| e.to_string())?;
                let manifest: iris_adapter_redis::MappingManifest =
                    von::from_str(&text).map_err(|e| e.to_string())?;
                println!(
                    "inspect source={source} backend=redis connectivity=ok mappings={} ({})",
                    manifest.tables.len(),
                    mapping_path.display()
                );
                for t in &manifest.tables {
                    println!(
                        "  - {} prefix={} pk={} encoding={:?}",
                        t.vos_table, t.key_prefix, t.primary_key_field, t.encoding
                    );
                }
            } else {
                println!(
                    "inspect source={source} backend=redis connectivity=ok \
                     (no mapping file at {}; run `iris adopt plan`)",
                    mapping_path.display()
                );
            }
        }
        other => {
            return Err(format!(
                "CLI live inspect does not support {:?} yet (use sqlite/postgres/mysql/redis)",
                other
            ));
        }
    }
    Ok(())
}

fn cmd_adopt_plan(
    project_dir: &Path,
    project: &IrisProject,
    source: &str,
    out: Option<&Path>,
) -> Result<(), String> {
    let ds = project.datasource(source).map_err(|e| e.to_string())?;
    if ds.mode != TruthMode::AdoptExisting && ds.mode != TruthMode::ManagedPush {
        eprintln!("warning: adopt plan on mode {:?}; continuing", ds.mode);
    }
    let schema = read_schema(project_dir, project)?;
    let out_path = out
        .map(Path::to_path_buf)
        .unwrap_or_else(|| default_mapping_path(project_dir, source));

    match ds.kind {
        DatasourceKind::Sqlite => {
            let path = resolve_path(project_dir, &expand_endpoint(ds, source)?);
            let db = SqliteSource::open(path).map_err(|e| e.to_string())?;
            let manifest = db.adopt(&schema).map_err(|e| e.to_string())?;
            write_relational_adopt(&out_path, &manifest)
        }
        DatasourceKind::Postgres => {
            let url = expand_endpoint(ds, source)?;
            let db = PostgresSource::connect(&url).map_err(|e| e.to_string())?;
            let manifest = db.adopt(&schema).map_err(|e| e.to_string())?;
            write_relational_adopt(&out_path, &manifest)
        }
        DatasourceKind::Mysql => {
            let url = expand_endpoint(ds, source)?;
            let db = MysqlSource::connect(&url).map_err(|e| e.to_string())?;
            let manifest = db.adopt(&schema).map_err(|e| e.to_string())?;
            write_relational_adopt(&out_path, &manifest)
        }
        DatasourceKind::Redis => {
            // Draft from VOS only -- never SCAN Redis to invent catalog.
            let url = expand_endpoint(ds, source)?;
            RedisSource::ping_url(&url).map_err(|e| e.to_string())?;
            let manifest =
                RedisSource::draft_keyspace_mapping(&schema).map_err(|e| e.to_string())?;
            let text = von::to_string_indented(&manifest).map_err(|e| e.to_string())?;
            write_von_file(&out_path, &text)?;
            println!(
                "wrote redis keyspace adopt plan {} (tables={}) -- review before connect",
                out_path.display(),
                manifest.tables.len()
            );
            Ok(())
        }
        other => Err(format!("CLI adopt plan does not support {:?} yet", other)),
    }
}

fn cmd_migrate_plan(config: &Path, source: &str, out: Option<&Path>) -> Result<(), String> {
    let path = iris_tools::migrate_plan(config, source, out)?;
    println!("wrote migrate plan {}", path.display());
    Ok(())
}

fn cmd_migrate_apply(
    config: &Path,
    source: &str,
    plan_path: &Path,
    yes: bool,
) -> Result<(), String> {
    if !yes {
        return Err("refusing to apply without --yes".into());
    }
    let report = iris_tools::migrate_apply(config, source, plan_path)?;
    println!(
        "applied plan {} created_tables={:?}",
        report.plan_id, report.created_tables
    );
    Ok(())
}

fn cmd_migrate_verify(config: &Path, source: &str) -> Result<(), String> {
    iris_tools::migrate_verify(config, source)?;
    println!("verify ok: no drift for source={source}");
    Ok(())
}

fn resolve_topology_path(
    project_dir: Option<&Path>,
    project: Option<&IrisProject>,
    topology: &str,
) -> Result<PathBuf, String> {
    let path = PathBuf::from(topology);
    if path.exists() {
        return Ok(path);
    }
    if let (Some(dir), Some(proj)) = (project_dir, project)
        && let Ok(rel) = proj.topology_path(topology)
    {
        return Ok(resolve_path(dir, rel));
    }
    if let Some(dir) = project_dir {
        let candidate = dir.join(TOPOLOGY_DIR).join(format!("{topology}.von"));
        if candidate.exists() {
            return Ok(candidate);
        }
        let candidate = dir.join(topology);
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(format!(
        "topology `{topology}` not found (pass a path, iris.von topologies name, or topologies/<name>.von)"
    ))
}

fn parse_access(op: &str) -> Result<AccessKind, String> {
    match op {
        "identity_read" => Ok(AccessKind::IdentityRead),
        "filtered_query" => Ok(AccessKind::FilteredQuery),
        "search" => Ok(AccessKind::Search),
        "vector_nearest" => Ok(AccessKind::VectorNearest),
        "write" => Ok(AccessKind::Write),
        "bytes_range" => Ok(AccessKind::BytesRange),
        "effect" => Ok(AccessKind::Effect),
        other => Err(format!("unknown --op `{other}`")),
    }
}

fn parse_intent(intent: &str) -> Result<ConsistencyIntent, String> {
    if let Some(secs) = intent.strip_prefix("bounded_stale:") {
        let max_lag_secs: u64 = secs
            .parse()
            .map_err(|_| format!("invalid bounded_stale seconds `{secs}`"))?;
        return Ok(ConsistencyIntent::BoundedStale { max_lag_secs });
    }
    if let Some(component) = intent.strip_prefix("projection_required:") {
        return Ok(ConsistencyIntent::ProjectionRequired {
            component: component.to_string(),
        });
    }
    match intent {
        "authoritative" => Ok(ConsistencyIntent::Authoritative),
        "read_your_writes" => Ok(ConsistencyIntent::ReadYourWrites),
        "eventual" => Ok(ConsistencyIntent::Eventual),
        other => Err(format!("unknown --intent `{other}`")),
    }
}

fn cmd_topology_verify(
    project_dir: Option<&Path>,
    project: Option<&IrisProject>,
    topology: &str,
) -> Result<(), String> {
    let path = resolve_topology_path(project_dir, project, topology)?;
    let topo = TopologyContract::load(&path).map_err(|e| e.to_string())?;
    for line in verify_report(&topo).map_err(|e| e.to_string())? {
        println!("{line}");
    }
    println!("verified {}", path.display());
    Ok(())
}

fn cmd_topology_activate(
    project_dir: Option<&Path>,
    project: Option<&IrisProject>,
    topology: &str,
    state_dir: Option<&Path>,
    force: bool,
    out: Option<&Path>,
) -> Result<(), String> {
    let path = resolve_topology_path(project_dir, project, topology)?;
    let topo = TopologyContract::load(&path).map_err(|e| e.to_string())?;
    let state = state_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| project_dir.unwrap_or(Path::new(".")).join(TOPOLOGY_DIR));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let report = activate_topology(&topo, &state, now, force).map_err(|e| e.to_string())?;
    let text = von::to_string_indented(&report).map_err(|e| e.to_string())?;
    assert_explain_safe(&text)?;
    let out_path = out.map(Path::to_path_buf).unwrap_or_else(|| {
        project_dir
            .unwrap_or(Path::new("."))
            .join("plans")
            .join(format!("{}-activate.von", topo.id))
    });
    write_von_file(&out_path, &text)?;
    if report.ok {
        println!(
            "topology activate ok id={} version={} state={}",
            topo.id,
            topo.topology_version,
            report.state_path.as_deref().unwrap_or("-")
        );
        println!("wrote {}", out_path.display());
        Ok(())
    } else {
        for n in &report.notes {
            eprintln!("{n}");
        }
        Err("topology activate refused".into())
    }
}

fn cmd_topology_plan(
    project_dir: Option<&Path>,
    project: Option<&IrisProject>,
    topology: &str,
    op: &str,
    intent: &str,
    table: Option<&str>,
    out: Option<&Path>,
) -> Result<(), String> {
    let path = resolve_topology_path(project_dir, project, topology)?;
    let topo = TopologyContract::load(&path).map_err(|e| e.to_string())?;
    let access = parse_access(op)?;
    let consistency = parse_intent(intent)?;
    let plan = topo
        .plan(access, consistency, table)
        .map_err(|e| e.to_string())?;
    let text = von::to_string_indented(&plan).map_err(|e| e.to_string())?;
    // Never print secrets; plans must not contain them. Still refuse SQL-ish content.
    if text.to_ascii_lowercase().contains("create table")
        || text.to_ascii_lowercase().contains("select ")
    {
        return Err("refusing to emit plan containing SQL-shaped text".into());
    }
    let out_path = out.map(Path::to_path_buf).unwrap_or_else(|| {
        let dir = project_dir.unwrap_or(Path::new("."));
        dir.join("plans").join(format!(
            "{}-{}-{}.von",
            topo.id,
            op,
            intent.replace(':', "_")
        ))
    });
    write_von_file(&out_path, &text)?;
    if plan.rejected {
        println!(
            "wrote rejected composite plan {} ({})",
            out_path.display(),
            plan.rejection.as_deref().unwrap_or("rejected")
        );
        return Err("composite plan rejected".into());
    }
    println!(
        "wrote composite plan {} (steps={}, authority={})",
        out_path.display(),
        plan.steps.len(),
        plan.authority_id
    );
    Ok(())
}

fn cmd_explain(
    project_dir: Option<&Path>,
    project: Option<&IrisProject>,
    topology: &str,
    op: &str,
    intent: &str,
    table: Option<&str>,
    vos: Option<&str>,
    out: Option<&Path>,
) -> Result<(), String> {
    let path = resolve_topology_path(project_dir, project, topology)?;
    let topo = TopologyContract::load(&path).map_err(|e| e.to_string())?;
    let access = parse_access(op)?;
    let consistency = parse_intent(intent)?;

    let physical = match vos {
        Some(src) => {
            let authority = topo.authority_id().map_err(|e| e.to_string())?;
            let adapter = topo
                .components
                .get(authority)
                .map(|c| c.adapter.as_str())
                .unwrap_or("reference");
            let caps = caps_for_adapter_sketch(adapter);
            Some(match Planner::new(caps).plan_source(src) {
                Ok(plan) => physical_explain_from_plan(&plan, adapter),
                Err(e) => PhysicalExplain {
                    rejected: true,
                    backend_sketch: adapter.into(),
                    nodes: Vec::new(),
                    note: Some(e.to_string().chars().take(160).collect()),
                },
            })
        }
        None => None,
    };

    let report =
        explain_topology(&topo, access, consistency, table, physical).map_err(|e| e.to_string())?;
    let text = von::to_string_indented(&report).map_err(|e| e.to_string())?;
    assert_explain_safe(&text)?;

    let out_path = out.map(Path::to_path_buf).unwrap_or_else(|| {
        let dir = project_dir.unwrap_or(Path::new("."));
        dir.join("plans").join(format!(
            "{}-explain-{}-{}.von",
            topo.id,
            op,
            intent.replace(':', "_")
        ))
    });
    write_von_file(&out_path, &text)?;
    println!(
        "wrote explain {} (rejected={}, steps={}, fallbacks={}, freshness_proven={})",
        out_path.display(),
        report.rejected,
        report.steps.len(),
        report.fallbacks.len(),
        report.freshness_proven
    );
    if report.rejected {
        return Err(report
            .rejection
            .unwrap_or_else(|| "explain plan rejected".into()));
    }
    Ok(())
}

fn caps_for_adapter_sketch(adapter: &str) -> CapabilitySet {
    match adapter {
        "sqlite" => SqliteSource::capabilities(),
        "postgres" => PostgresSource::capabilities(),
        "mysql" => MysqlSource::capabilities(),
        "redis" => RedisSource::capabilities(),
        // YYDB sketch uses reference-full caps; CLI does not link the native connector here.
        "yydb" | _ => CapabilitySet::reference_full(),
    }
}

fn cmd_projection_status(
    project_dir: Option<&Path>,
    project: Option<&IrisProject>,
    topology: &str,
    component: Option<&str>,
    live: bool,
    out: Option<&Path>,
) -> Result<(), String> {
    let path = resolve_topology_path(project_dir, project, topology)?;
    let topo = TopologyContract::load(&path).map_err(|e| e.to_string())?;

    let report = if live {
        let project = project.ok_or_else(|| {
            "projection status --live requires iris.von with datasource bindings".to_string()
        })?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        // Keep probes alive for the status call.
        let mut owned: Vec<(String, RedisWatermarkProbe)> = Vec::new();
        for (id, comp) in &topo.components {
            if comp.role != iris::ComponentRole::Cache {
                continue;
            }
            if let Some(filter) = component {
                if id != filter {
                    continue;
                }
            }
            let Some(ds_name) = comp.datasource.as_deref() else {
                continue;
            };
            let ds = project.datasource(ds_name).map_err(|e| e.to_string())?;
            if ds.kind != DatasourceKind::Redis {
                return Err(format!(
                    "Cache component `{id}` datasource `{ds_name}` must be kind=redis for live status (got {:?})",
                    ds.kind
                ));
            }
            let url = expand_endpoint(ds, ds_name)?;
            owned.push((id.clone(), RedisWatermarkProbe::new(url)));
        }

        let probes: std::collections::BTreeMap<String, &dyn CacheWatermarkProbe> = owned
            .iter()
            .map(|(id, p)| (id.clone(), p as &dyn CacheWatermarkProbe))
            .collect();

        let mut report =
            projection_status(&topo, Some(&probes), Some(now)).map_err(|e| e.to_string())?;
        if let Some(filter) = component {
            report.projections.retain(|p| p.component == filter);
            if report.projections.is_empty() {
                return Err(format!(
                    "no Cache component `{filter}` in topology `{}`",
                    topo.id
                ));
            }
        }
        report
    } else {
        let mut report = projection_status_offline(&topo).map_err(|e| e.to_string())?;
        if let Some(filter) = component {
            report.projections.retain(|p| p.component == filter);
            if report.projections.is_empty() {
                return Err(format!(
                    "no Cache component `{filter}` in topology `{}`",
                    topo.id
                ));
            }
        }
        report
    };

    let text = von::to_string_indented(&report).map_err(|e| e.to_string())?;
    assert_explain_safe(&text)?;

    let out_path = out.map(Path::to_path_buf).unwrap_or_else(|| {
        let dir = project_dir.unwrap_or(Path::new("."));
        let suffix = component.unwrap_or("cache");
        dir.join("plans")
            .join(format!("{}-projection-status-{suffix}.von", topo.id))
    });
    write_von_file(&out_path, &text)?;

    for row in &report.projections {
        let live_note = match &row.live {
            Some(v) if v.reachable => format!(
                "live=ok seq={} lag_secs={}",
                v.seq.map(|s| s.to_string()).unwrap_or_else(|| "-".into()),
                v.wall_lag_secs
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "-".into())
            ),
            Some(_) => "live=unreachable".into(),
            None => "live=offline".into(),
        };
        println!(
            "projection {} role={} adapter={} ds={} {live_note}",
            row.component,
            row.role,
            row.adapter,
            row.datasource.as_deref().unwrap_or("-")
        );
    }
    println!(
        "wrote projection status {} (projections={})",
        out_path.display(),
        report.projections.len()
    );
    Ok(())
}

fn cmd_object_status(root: &Path, out: Option<&Path>) -> Result<(), String> {
    let store = FsObjectStore::open(
        root,
        ObjectPolicy {
            hash_alg: Some(OBJECT_HASH_ALG_BLAKE3.into()),
            orphan_ttl_secs: None,
            pending_ttl_secs: None,
        },
    )
    .map_err(|e| e.to_string())?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let report = store.status_report(now).map_err(|e| e.to_string())?;
    let text = von::to_string_indented(&report).map_err(|e| e.to_string())?;
    assert_explain_safe(&text)?;
    let out_path = out
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.join("object-store-status.von"));
    write_von_file(&out_path, &text)?;
    println!(
        "object store root={} pending={} verified={} committed={} deleting={} aborted={} dangling_refs={}",
        report.root,
        report.counts.pending,
        report.counts.verified,
        report.counts.committed,
        report.counts.deleting,
        report.counts.aborted,
        report.dangling_refs
    );
    println!("wrote {}", out_path.display());
    Ok(())
}

fn cmd_object_gc(root: &Path, yes: bool) -> Result<(), String> {
    if !yes {
        return Err("object gc requires --yes (removes aborted/deleting/orphan pending)".into());
    }
    let store = FsObjectStore::open(
        root,
        ObjectPolicy {
            hash_alg: Some(OBJECT_HASH_ALG_BLAKE3.into()),
            orphan_ttl_secs: Some(0),
            pending_ttl_secs: Some(0),
        },
    )
    .map_err(|e| e.to_string())?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let removed = store.gc(now).map_err(|e| e.to_string())?;
    println!(
        "object gc root={} removed={}",
        root.display(),
        removed.len()
    );
    for id in removed {
        println!("  - {id}");
    }
    Ok(())
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ProjectionSeedFile {
    format: String,
    #[serde(default)]
    schema_fingerprint: Option<String>,
    documents: Vec<ProjectionDocument>,
}

fn cmd_projection_rebuild(
    project_dir: Option<&Path>,
    project: Option<&IrisProject>,
    topology: &str,
    component: &str,
    root: &Path,
    seed: &Path,
    schema_fingerprint: &str,
    yes: bool,
    out: Option<&Path>,
) -> Result<(), String> {
    if !yes {
        return Err(
            "projection rebuild requires --yes (fills a new generation then switches alias)".into(),
        );
    }
    let path = resolve_topology_path(project_dir, project, topology)?;
    let topo = TopologyContract::load(&path).map_err(|e| e.to_string())?;
    let comp = topo.components.get(component).ok_or_else(|| {
        format!(
            "component `{component}` not found in topology `{}`",
            topo.id
        )
    })?;
    if !matches!(
        comp.role,
        ComponentRole::SearchProjection | ComponentRole::VectorProjection
    ) {
        return Err(format!(
            "projection rebuild targets SearchProjection/VectorProjection; `{component}` is {:?}",
            comp.role
        ));
    }

    let seed_text = std::fs::read_to_string(seed).map_err(|e| e.to_string())?;
    let seed_file: ProjectionSeedFile =
        von::from_str(&seed_text).map_err(|e| format!("parse seed: {e}"))?;
    if seed_file.format != "iris.projection_seed" {
        return Err(format!(
            "seed format must be iris.projection_seed (got {})",
            seed_file.format
        ));
    }
    let fp = if schema_fingerprint.is_empty() {
        seed_file
            .schema_fingerprint
            .clone()
            .unwrap_or_else(|| "unspecified".into())
    } else {
        schema_fingerprint.to_string()
    };

    let store =
        LocalProjectionStore::open(root, topo.projection.clone()).map_err(|e| e.to_string())?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let handle = store
        .begin_rebuild(component, &fp, now)
        .map_err(|e| e.to_string())?;
    for doc in seed_file.documents {
        let mut doc = doc;
        doc.schema_fingerprint = fp.clone();
        store
            .upsert_building(&handle, doc, now)
            .map_err(|e| e.to_string())?;
    }
    let validation = store
        .validate_building(&handle)
        .map_err(|e| e.to_string())?;
    if !validation.ok {
        let _ = store.abort_rebuild(&handle, now);
        return Err(format!(
            "rebuild validation failed: {}",
            validation.notes.join("; ")
        ));
    }
    store.activate(&handle, now).map_err(|e| e.to_string())?;
    let status = store.rebuild_status(component).map_err(|e| e.to_string())?;
    let text = von::to_string_indented(&status).map_err(|e| e.to_string())?;
    assert_explain_safe(&text)?;
    let out_path = out.map(Path::to_path_buf).unwrap_or_else(|| {
        let dir = project_dir.unwrap_or(Path::new("."));
        dir.join("plans")
            .join(format!("{}-rebuild-{component}.von", topo.id))
    });
    write_von_file(&out_path, &text)?;
    println!(
        "projection rebuild ok component={} generation={} docs={} active={:?}",
        component, handle.generation, validation.doc_count, status.active_generation
    );
    println!("wrote {}", out_path.display());
    Ok(())
}

fn cmd_projection_verify(
    project_dir: Option<&Path>,
    project: Option<&IrisProject>,
    topology: &str,
    component: &str,
    root: Option<&Path>,
    out: Option<&Path>,
) -> Result<(), String> {
    let path = resolve_topology_path(project_dir, project, topology)?;
    let topo = TopologyContract::load(&path).map_err(|e| e.to_string())?;
    let store;
    let store_ref = if let Some(root) = root {
        store =
            LocalProjectionStore::open(root, topo.projection.clone()).map_err(|e| e.to_string())?;
        Some(&store)
    } else {
        None
    };
    let report = verify_projection(&topo, component, store_ref).map_err(|e| e.to_string())?;
    let text = von::to_string_indented(&report).map_err(|e| e.to_string())?;
    assert_explain_safe(&text)?;
    let out_path = out.map(Path::to_path_buf).unwrap_or_else(|| {
        project_dir
            .unwrap_or(Path::new("."))
            .join("plans")
            .join(format!("{}-verify-{component}.von", topo.id))
    });
    write_von_file(&out_path, &text)?;
    println!(
        "projection verify component={} role={} ok={}",
        report.component, report.role, report.ok
    );
    for c in &report.checks {
        println!(
            "  [{}] {} -- {}",
            if c.ok { "ok" } else { "fail" },
            c.id,
            c.detail
        );
    }
    println!("wrote {}", out_path.display());
    if report.ok {
        Ok(())
    } else {
        Err("projection verify failed".into())
    }
}
