//! TypeScript generated-client emitter (Dejavu templates + deterministic Rust helpers).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::{Error, FieldModel, GenerationModel, MacroModel, Result, TableModel, render};

const SYNTHESIZE_TS: &str = include_str!("../templates/typescript/synthesize.ts");
const INPUTS_HEADER: &str = include_str!("../templates/typescript/inputs_header.ts");

/// Emit all TypeScript client files for a generation model.
pub fn emit_typescript_client(model: &GenerationModel) -> Result<Vec<(String, String)>> {
    let ctx = model.to_json();
    let entity_names: HashSet<&str> = model.tables.iter().map(|t| t.name.as_str()).collect();

    let macros = emit_macros(&model.macros).replace(
        "./synthesize.js",
        "./_internal/synthesize.js",
    );
    // Single operations.ts: keep IrisDbBinding import from macros; drop duplicate from db.
    let db = emit_db(model)
        .replace("import type { IrisDbBinding } from \"@yydb/iris/types\";\n", "")
        .replace("import { GeneratedMacros } from \"./macros.js\";\n", "")
        .replace("./synthesize.js", "./_internal/synthesize.js");
    let operations = format!("{macros}\n{db}");

    let mut files: Vec<(String, String)> = vec![
        ("metadata.ts".into(), render("metadata", &ctx)?),
        ("index.ts".into(), render("index", &ctx)?),
        ("node.ts".into(), render("node", &ctx)?),
        ("browser.ts".into(), render("browser", &ctx)?),
        (
            "_internal/synthesize.ts".into(),
            SYNTHESIZE_TS.to_string(),
        ),
        ("references.ts".into(), emit_references(model)),
        ("models.ts".into(), emit_models(model)),
        ("inputs.ts".into(), emit_inputs(model, &entity_names)),
        ("operations.ts".into(), operations),
        ("errors.ts".into(), emit_errors_ts()),
    ];

    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}

/// Write TypeScript client files into `{out_dir}/generated/iris/typescript/`.
pub fn write_typescript_client(model: &GenerationModel, out_dir: &Path) -> Result<Vec<PathBuf>> {
    let root = crate::typescript_target_dir(out_dir);
    std::fs::create_dir_all(root.join("_internal"))?;
    let mut written = Vec::new();
    for (name, content) in emit_typescript_client(model)? {
        let target = root.join(&name);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp_name = name.replace('/', "__");
        let tmp = root.join(format!("{tmp_name}.tmp"));
        std::fs::write(&tmp, content)?;
        std::fs::rename(&tmp, &target)?;
        written.push(target);
    }
    Ok(written)
}

fn emit_errors_ts() -> String {
    r#"/** Target-native error surface (shim until full IrisError mapping). */
export type IrisGeneratedError = {
    code: string;
    message: string;
    path?: string;
    span?: { start: number; end: number };
    cause?: unknown;
};
"#
    .into()
}

fn camel_case(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
    }
}

fn scalar_base_type(vos_type: &str) -> &'static str {
    let base = vos_type.trim_end_matches('?').trim_start_matches('&');
    match base {
        "bool" => "boolean",
        "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "f32" | "f64" => "number",
        _ => "string",
    }
}

fn model_field_type(table: &TableModel, field: &FieldModel) -> String {
    if let Some(ref target) = field.reference_target {
        let reference = format!("{target}Reference");
        return if field.optional {
            format!("{reference} | null")
        } else {
            reference
        };
    }
    if field.primary {
        return format!("{}Id", table.name);
    }
    let base = scalar_base_type(&field.vos_type);
    if field.optional {
        format!("{base} | null")
    } else {
        base.into()
    }
}

fn scalar_filter_type(table: &TableModel, field: &FieldModel) -> String {
    if field.primary {
        format!("{}Id", table.name)
    } else {
        scalar_base_type(&field.vos_type).into()
    }
}

fn filter_ops_for(scalar_ts: &str) -> String {
    format!(
        "| {{
    eq?: {scalar_ts};
    not?: {scalar_ts};
    gt?: {scalar_ts};
    gte?: {scalar_ts};
    lt?: {scalar_ts};
    lte?: {scalar_ts};
    contains?: string;
    startsWith?: string;
    endsWith?: string;
}}"
    )
}

fn primary_key_field(table: &TableModel) -> Option<&FieldModel> {
    table
        .fields
        .iter()
        .find(|f| f.primary)
        .or(table.fields.first())
}

fn emit_references(model: &GenerationModel) -> String {
    let mut blocks = Vec::new();
    for table in &model.tables {
        let pk = primary_key_field(table);
        let pk_name = pk.map(|f| f.name.as_str()).unwrap_or("id");
        let pk_vos = pk.map(|f| f.vos_type.as_str()).unwrap_or("utf8");
        let id_type = format!("{}Id", table.name);
        let ref_type = format!("{}Reference", table.name);
        let ref_input = format!("{}RefInput", table.name);
        let scalar = scalar_base_type(pk_vos);
        blocks.push(format!(
            r#"declare const __iris{table}IdBrand: unique symbol;

/** Branded primary key for {table}. */
export type {id_type} = {scalar} & {{
    readonly [__iris{table}IdBrand]: true;
}};

/** Stored reference identity for &{table} (not an ORM relation object). */
export type {ref_type} = {{
    readonly __irisRef: "{table}";
    {pk_name}: {id_type};
}};

/** Values accepted when writing &{table}. */
export type {ref_input} =
    | {id_type}
    | {{ {pk_name}: {id_type} }}
    | {ref_type};"#,
            table = table.name,
            id_type = id_type,
            scalar = scalar,
            ref_type = ref_type,
            ref_input = ref_input,
            pk_name = pk_name,
        ));
    }
    format!("{}\n", blocks.join("\n\n"))
}

fn reference_targets(model: &GenerationModel) -> HashSet<String> {
    let mut targets = HashSet::new();
    for table in &model.tables {
        targets.insert(table.name.clone());
        for field in &table.fields {
            if let Some(ref target) = field.reference_target {
                targets.insert(target.clone());
            }
        }
    }
    targets
}

fn emit_models(model: &GenerationModel) -> String {
    let needed = reference_targets(model);
    let ref_imports: Vec<String> = needed
        .iter()
        .flat_map(|name| vec![format!("{name}Id"), format!("{name}Reference")])
        .collect();

    let blocks: Vec<String> = model
        .tables
        .iter()
        .map(|table| {
            let fields: Vec<String> = table
                .fields
                .iter()
                .map(|field| format!("    {}: {};", field.name, model_field_type(table, field)))
                .collect();
            format!(
                "export interface {} {{\n{}\n}}",
                table.name,
                fields.join("\n")
            )
        })
        .collect();

    format!(
        "import type {{ {} }} from \"./references.js\";\n\n{}\n",
        ref_imports.join(", "),
        blocks.join("\n\n")
    )
}

fn emit_inputs(model: &GenerationModel, entity_names: &HashSet<&str>) -> String {
    let entity_union: String = if model.tables.is_empty() {
        "never".into()
    } else {
        model
            .tables
            .iter()
            .map(|t| format!("\"{}\"", t.name))
            .collect::<Vec<_>>()
            .join(" | ")
    };

    let entity_map: String = model
        .tables
        .iter()
        .map(|t| format!("    {}: {};", t.name, t.name))
        .collect::<Vec<_>>()
        .join("\n");

    let imports_models: String = model
        .tables
        .iter()
        .map(|t| format!("    {},", t.name))
        .collect::<Vec<_>>()
        .join("\n");

    let imports_refs: String = model
        .tables
        .iter()
        .flat_map(|t| {
            vec![
                format!("    {}Id,", t.name),
                format!("    {}RefInput,", t.name),
            ]
        })
        .collect::<Vec<_>>()
        .join("\n");

    let header = INPUTS_HEADER
        .replace("{{ENTITY_MAP}}", &entity_map)
        .replace("{{ENTITY_UNION}}", &entity_union);

    let blocks: Vec<String> = model
        .tables
        .iter()
        .map(|table| emit_entity_inputs(table, entity_names))
        .collect();

    format!(
        "import type {{\n{imports_models}\n}} from \"./models.js\";\nimport type {{\n{imports_refs}\n}} from \"./references.js\";\n\n{header}\n{}\n",
        blocks.join("\n")
    )
}

fn emit_entity_inputs(table: &TableModel, entity_names: &HashSet<&str>) -> String {
    let primary_fields: Vec<_> = table.fields.iter().filter(|f| f.primary).collect();
    let unique_where = if primary_fields.is_empty() {
        format!("    id?: {}Id;", table.name)
    } else {
        primary_fields
            .iter()
            .map(|field| format!("    {}?: {}Id;", field.name, table.name))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let where_fields: String = table
        .fields
        .iter()
        .filter_map(|field| emit_where_field(table, field, entity_names))
        .collect::<Vec<_>>()
        .join("\n");

    let select_fields: String = table
        .fields
        .iter()
        .filter_map(|field| emit_select_field(field, entity_names))
        .collect::<Vec<_>>()
        .join("\n");

    let create_fields: String = table
        .fields
        .iter()
        .filter_map(|field| emit_create_field(table, field, entity_names))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"
export type {name}Select = {{
{select_fields}
}};

export type {name}WhereInput = {{
{where_fields}
}};

export type {name}WhereUniqueInput = {{
{unique_where}
}};

export type {name}CreateInput = {{
{create_fields}
}};

export type {name}UpdateInput = {{
    [K in keyof {name}CreateInput]?: {name}CreateInput[K];
}};

export type {name}FindManyArgs = {{
    where?: {name}WhereInput;
    select?: {name}Select;
    take?: number;
}};

export type {name}FindUniqueArgs = {{
    where: {name}WhereUniqueInput;
    select?: {name}Select;
}};

export type {name}CreateArgs = {{
    data: {name}CreateInput;
    select?: {name}Select;
}};

export type {name}FindManyResult<A extends {name}FindManyArgs> = Array<
    SelectResultFor<"{name}", A["select"]>
>;

export type {name}FindUniqueResult<A extends {name}FindUniqueArgs> =
    SelectResultFor<"{name}", A["select"]> | null;

export type {name}CreateResult<A extends {name}CreateArgs> = SelectResultFor<
    "{name}",
    A["select"]
>;"#,
        name = table.name,
        select_fields = select_fields,
        where_fields = where_fields,
        unique_where = unique_where,
        create_fields = create_fields,
    )
}

fn emit_where_field(
    table: &TableModel,
    field: &FieldModel,
    entity_names: &HashSet<&str>,
) -> Option<String> {
    if let Some(ref target) = field.reference_target {
        if entity_names.contains(target.as_str()) {
            return Some(format!(
                "    {}?: WherePathFor<\"{}\">;",
                field.name, target
            ));
        }
        return None;
    }
    let scalar = scalar_filter_type(table, field);
    Some(format!(
        "    {}?: {} {};",
        field.name,
        scalar,
        filter_ops_for(&scalar)
    ))
}

fn emit_select_field(field: &FieldModel, entity_names: &HashSet<&str>) -> Option<String> {
    if let Some(ref target) = field.reference_target {
        if entity_names.contains(target.as_str()) {
            return Some(format!(
                "    {}?: boolean | SelectPathFor<\"{}\">;",
                field.name, target
            ));
        }
        return None;
    }
    Some(format!("    {}?: boolean;", field.name))
}

fn emit_create_field(
    table: &TableModel,
    field: &FieldModel,
    entity_names: &HashSet<&str>,
) -> Option<String> {
    if let Some(ref target) = field.reference_target {
        if !entity_names.contains(target.as_str()) {
            return None;
        }
        let req = if field.optional { "?" } else { "" };
        return Some(format!("    {}{}: {}RefInput;", field.name, req, target));
    }
    if field.optional {
        Some(format!(
            "    {}?: {};",
            field.name,
            model_field_type(table, field)
        ))
    } else {
        Some(format!(
            "    {}: {};",
            field.name,
            model_field_type(table, field)
        ))
    }
}

fn macro_return_ts(return_type: &str) -> String {
    let base = return_type.trim_end_matches('?');
    match base {
        "utf8" | "uuid" | "decimal" | "datetime" => "string".into(),
        "bool" => "boolean".into(),
        "unit" => "void".into(),
        other
            if other.contains("::")
                || other.chars().next().is_some_and(|c| c.is_ascii_uppercase()) =>
        {
            other.into()
        }
        _ => "unknown".into(),
    }
}

fn emit_macros(macros: &[MacroModel]) -> String {
    if macros.is_empty() {
        return r#"import type { IrisDbBinding } from "@yydb/iris/types";

/** Schema macros (empty for this project). */
export class GeneratedMacros {
    constructor(private readonly binding: IrisDbBinding) {}
}
"#
        .into();
    }

    let methods: String = macros
        .iter()
        .map(|macro_def| {
            let ret = macro_return_ts(&macro_def.return_type);
            let body = if ret == "void" {
                "await this.binding.execute(source, parameters)".into()
            } else {
                format!("(await this.binding.query(source, parameters)) as {ret}")
            };
            format!(
                r#"
    async {name}(...args: unknown[]): Promise<{ret}> {{
        const {{ source, parameters }} = synthesizeMacroCall("{name}", args);
        return {body};
    }}"#,
                name = macro_def.name,
                ret = ret,
                body = body,
            )
        })
        .collect();

    format!(
        r#"import type {{ IrisDbBinding }} from "@yydb/iris/types";
import {{ synthesizeMacroCall }} from "./synthesize.js";

/** Generated macro bindings from schema truth (not backend capability intersection). */
export class GeneratedMacros {{
    constructor(private readonly binding: IrisDbBinding) {{}}
{methods}
}}
"#
    )
}

fn emit_db(model: &GenerationModel) -> String {
    let input_imports: String = model
        .tables
        .iter()
        .flat_map(|table| {
            [
                format!("{}FindManyArgs", table.name),
                format!("{}FindManyResult", table.name),
                format!("{}FindUniqueArgs", table.name),
                format!("{}FindUniqueResult", table.name),
                format!("{}CreateArgs", table.name),
                format!("{}CreateResult", table.name),
            ]
        })
        .collect::<Vec<_>>()
        .join(", ");

    let delegates: String = model
        .tables
        .iter()
        .map(|table| {
            format!(
                "    readonly {}: {}Delegate;",
                camel_case(&table.name),
                table.name
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let delegate_classes: String = model
        .tables
        .iter()
        .map(|table| {
            format!(
                r#"
export class {name}Delegate {{
    constructor(private readonly binding: IrisDbBinding) {{}}

    async findMany<A extends {name}FindManyArgs>(
        args?: A,
    ): Promise<{name}FindManyResult<A>> {{
        const {{ source, parameters }} = synthesizeFindMany("{name}", args);
        const rows = await this.binding.query(source, parameters);
        return rows as {name}FindManyResult<A>;
    }}

    async findUnique<A extends {name}FindUniqueArgs>(
        args: A,
    ): Promise<{name}FindUniqueResult<A>> {{
        const {{ source, parameters }} = synthesizeFindUnique("{name}", args);
        const rows = (await this.binding.query(source, parameters)) as unknown[];
        return (rows[0] ?? null) as {name}FindUniqueResult<A>;
    }}

    async create<A extends {name}CreateArgs>(
        args: A,
    ): Promise<{name}CreateResult<A>> {{
        const {{ source, parameters }} = synthesizeCreate("{name}", args as {{ data: Record<string, unknown>; select?: Record<string, unknown> }});
        const row = await this.binding.query(source, parameters);
        return row as {name}CreateResult<A>;
    }}
}}"#,
                name = table.name,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let ctor_inits: String = model
        .tables
        .iter()
        .map(|table| {
            format!(
                "        this.{} = new {}Delegate(binding);",
                camel_case(&table.name),
                table.name
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"import type {{ IrisDbBinding }} from "@yydb/iris/types";
import {{ GeneratedMacros }} from "./macros.js";
import {{ synthesizeCreate, synthesizeFindMany, synthesizeFindUnique }} from "./synthesize.js";
import type {{
    {input_imports}
}} from "./inputs.js";
{delegate_classes}

export class DbClient {{
{delegates}
    readonly $macros: GeneratedMacros;
    private readonly binding: IrisDbBinding;

    constructor(binding: IrisDbBinding) {{
        this.binding = binding;
        this.$macros = new GeneratedMacros(binding);
{ctor_inits}
    }}

    /** Direct VOS DML. Caller declares result type; Rust validates semantics at runtime. */
    async $query<T = unknown>(
        source: string,
        parameters?: Readonly<Record<string, unknown>>,
    ): Promise<T> {{
        return (await this.binding.query(source, parameters)) as T;
    }}

    /** Direct VOS DDL. Success maps VOS unit to void. */
    async $execute(
        source: string,
        parameters?: Readonly<Record<string, unknown>>,
    ): Promise<void> {{
        await this.binding.execute(source, parameters);
    }}

    async $close(): Promise<void> {{
        await this.binding.close();
    }}
}}

/** Construct a client from an injected binding (Node/Web host wiring stays outside generated types). */
export function createClient(binding: IrisDbBinding): DbClient {{
    return new DbClient(binding);
}}
"#
    )
}

/// Unsupported generate target (caller-facing).
pub fn unsupported_target(target: &str) -> Error {
    Error::UnsupportedTarget(target.to_owned())
}
