//! Iris code generation (Dejavu-backed) for the **Rust host**.
//!
//! Shared `.dejavu` templates + GenerationModel live in the multi-language mono;
//! each host renders with its own Dejavu facade (this crate uses the Rust
//! `dejavu` crate). Emit Rust bindings that call `iris::*` -- never FFI wraps
//! for other languages. Other hosts (e.g. TypeScript `@yydb/iris`) ship their
//! own generator against the same template contract.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    sync::Mutex,
};

use dejavu::{Dejavu, IrDocument};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use vos::{CanonicalType, CanonicalTypeArgument, ContractDiagnostic, SchemaProjection, TypeContract, TypeContractKind};

/// Generator errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Unknown template stem.
    #[error("unknown dejavu template `{0}`")]
    UnknownTemplate(String),
    /// Dejavu render/parse failure.
    #[error("{0}")]
    Dejavu(String),
    /// Embedded AOT IR JSON could not be deserialized.
    #[error("invalid AOT IR for `{0}`: {1}")]
    InvalidIr(String, String),
    /// VOS schema parse failure.
    #[error("vos: {0}")]
    Vos(String),
    /// Unsupported VOS type for the Rust emitter.
    #[error("unsupported VOS type for Rust emit: {0}")]
    UnsupportedType(String),
    /// Unsupported generate target.
    #[error("unsupported generate target `{0}` (use rust or typescript)")]
    UnsupportedTarget(String),
    /// Two VOS fields map to the same TypeScript author name.
    #[error("TypeScript field naming collision on `{entity}`: `{wire_a}` and `{wire_b}` map to `{ts_name}`")]
    NamingCollision { entity: String, wire_a: String, wire_b: String, ts_name: String },
    /// I/O while writing generated files.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Result alias.
pub type Result<T> = std::result::Result<T, Error>;

mod carrier;
mod rust_view;
mod ts_view;
mod typescript;

include!(concat!(env!("OUT_DIR"), "/aot_registry.rs"));

pub use typescript::{emit_typescript_client, write_typescript_client};

/// One VOS field in the generation model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldModel {
    /// VOS-assigned durable field identity, absent only in legacy source models.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_id: Option<u64>,
    /// VOS-assigned virtual slot, absent only in legacy source models.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub virtual_field_index: Option<u32>,
    /// Field name.
    pub name: String,
    /// Rust type text for the emitter.
    pub rust_ty: String,
    /// VOS type label.
    pub vos_type: String,
    /// Primary key marker.
    pub primary: bool,
    /// Optional / nullable.
    pub optional: bool,
    /// True when the VOS type is `uuid` (including `uuid?`).
    #[serde(default)]
    pub is_uuid: bool,
    /// Target entity for `&T` reference fields (`None` for scalars).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_target: Option<String>,
}

/// One VOS macro parameter in the generation model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroParamModel {
    /// Parameter name.
    pub name: String,
    /// VOS type label.
    pub vos_type: String,
    /// TypeScript parameter type.
    pub ts_type: String,
}

/// One VOS macro in the generation model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroModel {
    /// Macro name in schema.
    pub name: String,
    /// Return type label (VOS surface).
    pub return_type: String,
    /// Positional parameters from the macro signature.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<MacroParamModel>,
}

/// One VOS table in the generation model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableModel {
    /// VOS-assigned durable type identity, absent only in legacy source models.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_id: Option<u64>,
    /// VOS table name.
    pub name: String,
    /// Rust struct name (same as table for Phase 8).
    pub rust_type: String,
    /// Fields.
    pub fields: Vec<FieldModel>,
}

/// Versioned, deterministic generation input (templates must not see raw AST).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationModel {
    /// Generator crate version.
    pub generator_version: String,
    /// Schema fingerprint.
    pub schema_fingerprint: String,
    /// Tables.
    pub tables: Vec<TableModel>,
    /// Schema macros (always emitted in generated TS regardless of backend capability).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub macros: Vec<MacroModel>,
}

impl GenerationModel {
    /// Builds from a strict VOS resolved artifact without parsing source or allocating identities.
    pub fn from_resolved_artifact(artifact: &str) -> Result<Self> {
        let contract = vos::ResolvedContract::from_json(artifact).map_err(|error| Error::Vos(format!("{}: {}", error.code, error.message)))?;
        let mut tables = Vec::new();
        for item in &contract.types {
            if item.kind != TypeContractKind::Table || item.canonical_path.len() != 1 {
                return Err(Error::UnsupportedType(format!("resolved entity {}", item.canonical_path.join("::"))));
            }
            let mut fields = Vec::new();
            for field in &item.fields {
                let mapped = map_resolved_field_type(&field.canonical_type)?;
                fields.push(FieldModel {
                    field_id: Some(field.field_id),
                    virtual_field_index: field.virtual_field_index,
                    name: field.canonical_name.clone(),
                    rust_ty: mapped.rust_ty,
                    vos_type: mapped.vos_type,
                    primary: field.attributes.iter().any(|attribute| attribute.name == "primary"),
                    optional: mapped.optional,
                    is_uuid: mapped.is_uuid,
                    reference_target: mapped.reference_target,
                });
            }
            fields.sort_by_key(|field| field.virtual_field_index);
            tables.push(TableModel {
                type_id: Some(item.type_id),
                name: item.canonical_path[0].clone(),
                rust_type: item.canonical_path[0].clone(),
                fields,
            });
        }
        tables.sort_by_key(|table| table.type_id);
        Ok(Self {
            generator_version: env!("CARGO_PKG_VERSION").into(),
            schema_fingerprint: contract.schema_fingerprint,
            tables,
            macros: Vec::new(),
        })
    }

    /// Build from a VOS schema document string.
    pub fn from_vos_schema(source: &str) -> Result<Self> {
        let input = vos::parse_oak(source).map_err(Error::Vos)?;
        if !source.trim().is_empty() && input.declarations().is_empty() {
            return Err(Error::Vos("schema document contains no VOS declarations".into()));
        }
        Self::from_document(&input)
    }

    /// Build from a parsed VOS contract input.
    pub fn from_document(input: &vos::ContractInput) -> Result<Self> {
        let projection = project_schema(input)?;
        let operations = input.project_operations().map_err(|diagnostics| Error::Vos(format_diagnostics(&diagnostics)))?;
        let mut tables = Vec::new();
        for table in projection.types.iter().filter(|item| item.kind == TypeContractKind::Table) {
            ensure_single_primary(table)?;
            let mut fields = Vec::new();
            for field in &table.fields {
                let mapped = map_field_type(&field.canonical_type)?;
                fields.push(FieldModel {
                    field_id: None,
                    virtual_field_index: None,
                    name: field.canonical_name.clone(),
                    rust_ty: mapped.rust_ty,
                    vos_type: mapped.vos_type,
                    primary: field.attributes.iter().any(|attribute| attribute.name == "primary"),
                    optional: mapped.optional,
                    is_uuid: mapped.is_uuid,
                    reference_target: mapped.reference_target,
                });
            }
            let Some(name) = table.canonical_path.last()
            else {
                return Err(Error::UnsupportedType("table declaration without a name".into()));
            };
            tables.push(TableModel { type_id: None, name: name.clone(), rust_type: name.clone(), fields });
        }
        let mut macros = Vec::new();
        for operation in operations.iter().filter(|operation| operation.kind == vos::OperationKind::Macro) {
            let params = operation
                .parameters
                .iter()
                .map(|param| {
                    let vos_type = type_label(&param.type_expr);
                    MacroParamModel { name: param.name.clone(), vos_type: vos_type.clone(), ts_type: carrier::vos_type_to_ts(&vos_type) }
                })
                .collect();
            macros.push(MacroModel {
                name: operation.name.clone(),
                return_type: operation.return_type.as_ref().map(type_label).unwrap_or_else(|| "unit".into()),
                params,
            });
        }
        Ok(Self {
            generator_version: env!("CARGO_PKG_VERSION").into(),
            schema_fingerprint: fingerprint_schema(&projection, &operations),
            tables,
            macros,
        })
    }

    /// JSON value for Dejavu contexts.
    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).expect("GenerationModel serializes")
    }
}

/// Render a registered template by stem name.
pub fn render(name: &str, ctx: &Value) -> Result<String> {
    render_registered(name, ctx)
}

/// Whether this build prefers AOT.
pub const fn prefers_aot() -> bool {
    cfg!(feature = "aot")
}

/// Default TypeScript generate root when `iris.config.ts` `generate.out` is `.` (legacy).
pub const DEFAULT_TYPESCRIPT_GENERATE_OUT: &str = "src/generated/iris";

/// Resolve `generate.out` (project-relative) to the TypeScript client root.
pub fn resolve_typescript_generate_root(project_root: &std::path::Path, generate_out: &str) -> std::path::PathBuf {
    let relative = if generate_out == "." { DEFAULT_TYPESCRIPT_GENERATE_OUT } else { generate_out };
    project_root.join(relative)
}

/// Output directory for the Rust target (`{generate_root}/rust/`).
pub fn rust_target_dir(generate_root: &std::path::Path) -> std::path::PathBuf {
    generate_root.join("rust")
}

/// Output directory for the TypeScript target (`generate.out` from `iris.config.ts`).
pub fn typescript_target_dir(generate_root: &std::path::Path) -> std::path::PathBuf {
    generate_root.to_path_buf()
}

fn rust_file_header(model: &GenerationModel) -> Result<String> {
    render(
        "rust/file_header",
        &json!({
            "generator_version": model.generator_version,
            "schema_fingerprint": model.schema_fingerprint,
        }),
    )
}

/// Emit Rust domain + client as a single string (tests / inspection).
pub fn emit_rust_domain(model: &GenerationModel) -> Result<String> {
    let files = emit_rust_files(model)?;
    let mut combined = String::new();
    for (name, content) in files {
        combined.push_str(&format!("\n// ===== {name} =====\n"));
        combined.push_str(&content);
    }
    Ok(combined)
}

/// Emit Rust target files: `mod` / `models` / `operations` / `metadata` / `errors`.
pub fn emit_rust_files(model: &GenerationModel) -> Result<Vec<(String, String)>> {
    let ctx = model.rust_template_context()?;
    let header = rust_file_header(model)?;
    let structs = unescape_rust_template(&render("rust/domain_mod", &model.to_json())?);
    let helpers = render("rust/models_helpers", &ctx)?;
    let impls = unescape_rust_template(&render("rust/models_impl", &ctx)?);
    let models = format!("{header}\n{structs}\n{helpers}\n{impls}");
    let metadata = format!("{header}\n{}\n", render("rust/metadata", &ctx)?);
    let errors = format!("{header}\n{}\n", render("rust/errors", &ctx)?);
    let operations = format!("{header}\n{}\n", unescape_rust_template(&render("rust/operations", &ctx)?),);
    let index = format!("{header}\n{}\n", render("rust/rust_index", &ctx)?);

    Ok(vec![
        ("mod.rs".into(), index),
        ("models.rs".into(), models),
        ("operations.rs".into(), operations),
        ("metadata.rs".into(), metadata),
        ("errors.rs".into(), errors),
    ])
}

/// Dejavu template mode HTML-escapes output; generated source must be unescaped.
pub(crate) fn unescape_dejavu_template(text: &str) -> String {
    text.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'")
}

/// Rust-only alias (angle brackets in generics and lifetimes).
fn unescape_rust_template(text: &str) -> String {
    unescape_dejavu_template(text)
}

fn write_files_atomic(root: &std::path::Path, files: Vec<(String, String)>) -> Result<Vec<std::path::PathBuf>> {
    std::fs::create_dir_all(root)?;
    let mut written = Vec::new();
    for (name, content) in files {
        let target = root.join(&name);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = root.join(format!("{name}.tmp"));
        std::fs::write(&tmp, content)?;
        std::fs::rename(&tmp, &target)?;
        written.push(target);
    }
    Ok(written)
}

/// Write Rust bindings into `{generate_root}/rust/`.
pub fn write_rust_domain(model: &GenerationModel, generate_root: &std::path::Path) -> Result<Vec<std::path::PathBuf>> {
    let root = rust_target_dir(generate_root);
    write_files_atomic(&root, emit_rust_files(model)?)
}

/// Generate bindings for `target` (`rust` | `typescript`) under `generate_root`.
pub fn generate(model: &GenerationModel, target: &str, generate_root: &std::path::Path) -> Result<Vec<std::path::PathBuf>> {
    match target {
        "rust" => write_rust_domain(model, generate_root),
        "typescript" | "ts" => write_typescript_client(model, generate_root),
        other => Err(typescript::unsupported_target(other)),
    }
}

/// Parse schema source and generate bindings for `target`.
pub fn generate_from_source(source: &str, target: &str, generate_root: &std::path::Path) -> Result<(GenerationModel, Vec<std::path::PathBuf>)> {
    let model = GenerationModel::from_vos_schema(source)?;
    let paths = generate(&model, target, generate_root)?;
    Ok((model, paths))
}

struct MappedFieldType {
    rust_ty: String,
    vos_type: String,
    optional: bool,
    is_uuid: bool,
    reference_target: Option<String>,
}

fn type_label(ty: &CanonicalType) -> String {
    match ty {
        CanonicalType::Named(path) => path.join("::"),
        CanonicalType::Optional(inner) => format!("{}?", type_label(inner)),
        CanonicalType::List(inner) => format!("[{}]", type_label(inner)),
        CanonicalType::Reference(inner) => format!("&{}", type_label(inner)),
        CanonicalType::Generic { path, arguments } => {
            let arguments = arguments
                .iter()
                .map(|argument| match argument {
                    CanonicalTypeArgument::Type(value) => type_label(value),
                    CanonicalTypeArgument::Literal(value) => value.clone(),
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}<{arguments}>", path.join("::"))
        }
    }
}

fn map_resolved_field_type(ty: &vos::ResolvedCanonicalType) -> Result<MappedFieldType> {
    use vos::ResolvedCanonicalType;
    match ty {
        ResolvedCanonicalType::Optional(inner) => {
            let mut mapped = map_resolved_field_type(inner)?;
            mapped.optional = true;
            mapped.vos_type.push('?');
            mapped.rust_ty = format!("Option<{}>", mapped.rust_ty);
            Ok(mapped)
        }
        ResolvedCanonicalType::Reference(inner) => {
            let target = reference_target_from_resolved(inner)?;
            Ok(MappedFieldType {
                rust_ty: format!("{target}Ref"),
                vos_type: format!("&{target}"),
                optional: false,
                is_uuid: false,
                reference_target: Some(target),
            })
        }
        ResolvedCanonicalType::Builtin(path) if path.len() == 1 => {
            let name = path[0].as_str();
            let label = carrier::faithful_label(name).ok_or_else(|| Error::UnsupportedType(name.to_owned()))?;
            let rust_ty = carrier::rust_carrier_for_label(label).ok_or_else(|| Error::UnsupportedType(name.to_owned()))?.to_owned();
            Ok(MappedFieldType { rust_ty, vos_type: label.to_owned(), optional: false, is_uuid: label == "uuid", reference_target: None })
        }
        ResolvedCanonicalType::User { path, .. } => {
            let name = path.join("::");
            Ok(MappedFieldType { rust_ty: name.clone(), vos_type: name, optional: false, is_uuid: false, reference_target: None })
        }
        other => Err(Error::UnsupportedType(format!("resolved type {other:?}"))),
    }
}

fn reference_target_from_resolved(ty: &vos::ResolvedCanonicalType) -> Result<String> {
    use vos::ResolvedCanonicalType;
    match ty {
        ResolvedCanonicalType::User { path, .. } => Ok(path.join("::")),
        ResolvedCanonicalType::Builtin(path) => Ok(path.join("::")),
        other => Err(Error::UnsupportedType(format!("resolved reference target `{other:?}`"))),
    }
}

fn map_field_type(ty: &CanonicalType) -> Result<MappedFieldType> {
    match ty {
        CanonicalType::Optional(inner) => {
            let mut mapped = map_field_type(inner)?;
            mapped.optional = true;
            mapped.vos_type = format!("{}?", mapped.vos_type);
            mapped.rust_ty = format!("Option<{}>", mapped.rust_ty);
            Ok(mapped)
        }
        CanonicalType::Reference(inner) => {
            let target = reference_target_name(inner)?;
            Ok(MappedFieldType {
                rust_ty: format!("{target}Ref"),
                vos_type: format!("&{target}"),
                optional: false,
                is_uuid: false,
                reference_target: Some(target),
            })
        }
        CanonicalType::Named(path) if path.len() == 1 => {
            let name = path[0].as_str();
            match carrier::builtin_rust_and_label(name) {
                Some((rust, vos)) => {
                    Ok(MappedFieldType { rust_ty: rust.into(), is_uuid: vos == "uuid", vos_type: vos, optional: false, reference_target: None })
                }
                None => Ok(MappedFieldType {
                    rust_ty: name.to_owned(),
                    vos_type: name.to_owned(),
                    optional: false,
                    is_uuid: false,
                    reference_target: None,
                }),
            }
        }
        CanonicalType::Named(path) => {
            Ok(MappedFieldType { rust_ty: path.join("::"), vos_type: path.join("::"), optional: false, is_uuid: false, reference_target: None })
        }
        other => Err(Error::UnsupportedType(format!("{other:?}"))),
    }
}

fn reference_target_name(ty: &CanonicalType) -> Result<String> {
    match ty {
        CanonicalType::Named(path) => Ok(path.join("::")),
        other => Err(Error::UnsupportedType(format!("reference target `{other:?}`"))),
    }
}

fn ensure_single_primary(table: &TypeContract) -> Result<()> {
    let primary_count = table.fields.iter().flat_map(|field| field.attributes.iter()).filter(|attribute| attribute.name == "primary").count();
    if primary_count != 1 {
        return Err(Error::Vos(format!("table `{}` requires exactly one primary field", table.canonical_path.join("::"))));
    }
    Ok(())
}

/// Project only schema declarations through the VOS schema contract.
///
/// Operation declarations (for example durable macros) stay in the Oak input
/// and are projected separately, so they never block table/class projection.
fn project_schema(input: &vos::ContractInput) -> Result<SchemaProjection> {
    use vos::syntax::VosDeclarationKind;
    let root = input.clone().into_oak();
    let declarations = root
        .declarations
        .iter()
        .filter(|declaration| matches!(declaration.kind, VosDeclarationKind::Namespace | VosDeclarationKind::Table | VosDeclarationKind::Class))
        .cloned()
        .collect();
    vos::ContractInput::from_oak(vos::syntax::VosRoot { source: root.source, syntax: root.syntax, declarations })
        .project_schema()
        .map_err(|diagnostics| Error::Vos(format_diagnostics(&diagnostics)))
}

fn format_diagnostics(diagnostics: &[ContractDiagnostic]) -> String {
    diagnostics.iter().map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message)).collect::<Vec<_>>().join("\n")
}

fn fingerprint_schema(projection: &SchemaProjection, operations: &[vos::OperationProjection]) -> String {
    let mut h = DefaultHasher::new();
    format!("{projection:?}").hash(&mut h);
    format!("{operations:?}").hash(&mut h);
    format!("{:x}", h.finish())
}

#[allow(dead_code)]
fn render_aot_ir(ir_json: &'static str, ctx: &Value) -> Result<String> {
    static CACHE: OnceCell<Mutex<HashMap<usize, IrDocument>>> = OnceCell::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = ir_json.as_ptr() as usize;
    let doc = {
        let mut guard = cache.lock().expect("ir cache lock");
        if let Some(doc) = guard.get(&key) {
            doc.clone()
        }
        else {
            let doc: IrDocument = serde_json::from_str(ir_json).map_err(|e| Error::InvalidIr("embedded".into(), e.to_string()))?;
            guard.insert(key, doc.clone());
            doc
        }
    };
    Dejavu::render(&doc, ctx).map_err(|e| Error::Dejavu(e.to_string()))
}

#[allow(dead_code)]
fn render_dyn_source(source: &str, ctx: &Value) -> Result<String> {
    Dejavu::render_source(source, ctx).map_err(|e| Error::Dejavu(format!("{e:?}")))
}

/// Marker kept for older Phase 0 callers.
pub fn is_stub() -> bool {
    false
}
