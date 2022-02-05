//! Iris code generation (Dejavu-backed) for the **Rust host**.
//!
//! Shared `.dejavu` templates + GenerationModel live in the multi-language mono;
//! each host renders with its own Dejavu facade (this crate uses the Rust
//! `dejavu` crate). Emit Rust bindings that call `iris::*` -- never FFI wraps
//! for other languages. Other hosts (e.g. TypeScript `@yydb/iris`) ship their
//! own generator against the same template contract.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

use dejavu::{Dejavu, IrDocument};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use vos::ast::{BuiltinType, Document, Item, TypeExpr};

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
    /// I/O while writing generated files.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Result alias.
pub type Result<T> = std::result::Result<T, Error>;

include!(concat!(env!("OUT_DIR"), "/aot_registry.rs"));

/// One VOS field in the generation model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldModel {
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
    /// Target entity for `&T` reference fields (`None` for scalars).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_target: Option<String>,
}

/// One VOS macro in the generation model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroModel {
    /// Macro name in schema.
    pub name: String,
    /// Return type label (VOS surface).
    pub return_type: String,
}

/// One VOS table in the generation model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableModel {
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
    /// Build from a VOS schema document string.
    pub fn from_vos_schema(source: &str) -> Result<Self> {
        let document = vos::parser::parse_document(source).map_err(|d| {
            Error::Vos(
                d.errors
                    .first()
                    .map(|e| e.message.clone())
                    .unwrap_or_else(|| "parse failed".into()),
            )
        })?;
        Self::from_document(&document)
    }

    /// Build from a parsed VOS document.
    pub fn from_document(document: &Document) -> Result<Self> {
        let mut tables = Vec::new();
        let mut macros = Vec::new();
        for item in &document.items {
            match item {
                Item::Table(table) => {
                    let mut fields = Vec::new();
                    for field in &table.fields {
                        let mapped = map_field_type(&field.ty)?;
                        fields.push(FieldModel {
                            name: field.name.clone(),
                            rust_ty: mapped.rust_ty,
                            vos_type: mapped.vos_type,
                            primary: field.is_primary(),
                            optional: mapped.optional,
                            reference_target: mapped.reference_target,
                        });
                    }
                    tables.push(TableModel {
                        name: table.name.clone(),
                        rust_type: table.name.clone(),
                        fields,
                    });
                }
                Item::Macro(macro_def) => {
                    macros.push(MacroModel {
                        name: macro_def.name.clone(),
                        return_type: macro_def
                            .return_ty
                            .as_ref()
                            .map(type_label)
                            .unwrap_or_else(|| "unit".into()),
                    });
                }
                _ => {}
            }
        }
        Ok(Self {
            generator_version: env!("CARGO_PKG_VERSION").into(),
            schema_fingerprint: fingerprint_document(document),
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

/// Emit Rust domain module text for a generation model.
pub fn emit_rust_domain(model: &GenerationModel) -> Result<String> {
    let header = render(
        "file_header",
        &json!({
            "generator_version": model.generator_version,
            "schema_fingerprint": model.schema_fingerprint,
        }),
    )?;
    let body = render("domain_mod", &model.to_json())?;
    Ok(format!("{header}\n{}", unescape_rust_template(&body)))
}

/// Dejavu template mode HTML-escapes `<`/`>`; Rust types need raw angle brackets.
fn unescape_rust_template(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

/// Write Rust domain bindings into `out_dir` atomically (temp + rename).
pub fn write_rust_domain(
    model: &GenerationModel,
    out_dir: &std::path::Path,
) -> Result<std::path::PathBuf> {
    std::fs::create_dir_all(out_dir)?;
    let target = out_dir.join("mod.rs");
    let tmp = out_dir.join("mod.rs.tmp");
    let text = emit_rust_domain(model)?;
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, &target)?;
    Ok(target)
}

struct MappedFieldType {
    rust_ty: String,
    vos_type: String,
    optional: bool,
    reference_target: Option<String>,
}

fn type_label(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Builtin(b) => format!("{b:?}").to_ascii_lowercase(),
        TypeExpr::Named(n) => n.clone(),
        TypeExpr::Optional(inner) => format!("{}?", type_label(inner)),
        TypeExpr::List(inner) => format!("[{}]", type_label(inner)),
        TypeExpr::Reference(inner) => format!("&{}", type_label(inner)),
        TypeExpr::Vector { dim } => format!("vector<{dim}>"),
        TypeExpr::File => "file".into(),
        other => format!("{other:?}"),
    }
}

fn map_field_type(ty: &TypeExpr) -> Result<MappedFieldType> {
    match ty {
        TypeExpr::Optional(inner) => {
            let mut mapped = map_field_type(inner)?;
            mapped.optional = true;
            mapped.vos_type = format!("{}?", mapped.vos_type);
            mapped.rust_ty = format!("Option<{}>", mapped.rust_ty);
            Ok(mapped)
        }
        TypeExpr::Reference(inner) => {
            let target = reference_target_name(inner)?;
            Ok(MappedFieldType {
                rust_ty: format!("{target}Ref"),
                vos_type: format!("&{target}"),
                optional: false,
                reference_target: Some(target),
            })
        }
        TypeExpr::Builtin(b) => {
            let (rust, vos) = map_builtin(b)?;
            Ok(MappedFieldType {
                rust_ty: rust.into(),
                vos_type: vos.into(),
                optional: false,
                reference_target: None,
            })
        }
        TypeExpr::Named(name) => Ok(MappedFieldType {
            rust_ty: name.clone(),
            vos_type: name.clone(),
            optional: false,
            reference_target: None,
        }),
        other => Err(Error::UnsupportedType(format!("{other:?}"))),
    }
}

fn reference_target_name(ty: &TypeExpr) -> Result<String> {
    match ty {
        TypeExpr::Named(name) => Ok(name.clone()),
        TypeExpr::Builtin(b) => Ok(format!("{b:?}").to_ascii_lowercase()),
        other => Err(Error::UnsupportedType(format!("reference target `{other:?}`"))),
    }
}

fn map_builtin(b: &BuiltinType) -> Result<(&'static str, &'static str)> {
    match b {
        BuiltinType::Bool => Ok(("bool", "bool")),
        BuiltinType::I8 => Ok(("i8", "i8")),
        BuiltinType::I16 => Ok(("i16", "i16")),
        BuiltinType::I32 => Ok(("i32", "i32")),
        BuiltinType::I64 => Ok(("i64", "i64")),
        BuiltinType::U8 => Ok(("u8", "u8")),
        BuiltinType::U16 => Ok(("u16", "u16")),
        BuiltinType::U32 => Ok(("u32", "u32")),
        BuiltinType::U64 => Ok(("u64", "u64")),
        BuiltinType::F32 => Ok(("f32", "f32")),
        BuiltinType::F64 => Ok(("f64", "f64")),
        BuiltinType::Utf8 | BuiltinType::Utf16 => Ok(("String", "utf8")),
        BuiltinType::Uuid => Ok(("String", "uuid")),
        BuiltinType::Decimal => Ok(("String", "decimal")),
        BuiltinType::Date | BuiltinType::Time | BuiltinType::DateTimeUtc => Ok(("String", "datetime")),
        BuiltinType::Bytes => Ok(("Vec<u8>", "bytes")),
        _ => Err(Error::UnsupportedType(format!("{b:?}"))),
    }
}

#[allow(dead_code)]
fn map_rust_type(ty: &TypeExpr) -> Result<(String, String, bool)> {
    let mapped = map_field_type(ty)?;
    Ok((mapped.rust_ty, mapped.vos_type, mapped.optional))
}

fn fingerprint_document(document: &Document) -> String {
    let mut h = DefaultHasher::new();
    format!("{document:?}").hash(&mut h);
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
        } else {
            let doc: IrDocument = serde_json::from_str(ir_json)
                .map_err(|e| Error::InvalidIr("embedded".into(), e.to_string()))?;
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
