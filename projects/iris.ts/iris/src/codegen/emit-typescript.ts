import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import type { SchemaIntrospection, SchemaTableModel } from "../types/schema-introspection.ts";
import { emitInputs } from "./emit-inputs.ts";
import { emitMacros } from "./emit-macros.ts";
import { emitReferences, modelFieldType, referenceTargets } from "./emit-references.ts";

export type EmitTypescriptOptions = {
    outDir: string;
    introspection: SchemaIntrospection;
};

export type EmitTypescriptResult = {
    files: string[];
};

function camelCase(name: string): string {
    return name.charAt(0).toLowerCase() + name.slice(1);
}

function emitModels(tables: SchemaTableModel[]): string {
    const neededRefs = referenceTargets(tables);
    for (const table of tables) {
        neededRefs.add(table.name);
    }

    const refImports = [...neededRefs]
        .sort()
        .flatMap((name) => [`${name}Id`, `${name}Reference`])
        .join(", ");

    const blocks = tables.map((table) => {
        const fields = table.fields
            .map((field) => `    ${field.name}: ${modelFieldType(table, field)};`)
            .join("\n");
        return `export interface ${table.name} {\n${fields}\n}`;
    });

    return `import type { ${refImports} } from "./references.js";\n\n${blocks.join("\n\n")}\n`;
}

function emitMetadata(intro: SchemaIntrospection): string {
    return `export const IRIS_SCHEMA_FINGERPRINT = "${intro.schemaFingerprint}";\nexport const IRIS_GENERATOR_VERSION = "${intro.generatorVersion}";\n`;
}

function emitDb(tables: SchemaTableModel[]): string {
    const inputImports = tables
        .flatMap((table) => [
            `${table.name}FindManyArgs`,
            `${table.name}FindManyResult`,
            `${table.name}FindUniqueArgs`,
            `${table.name}FindUniqueResult`,
            `${table.name}CreateArgs`,
            `${table.name}CreateResult`,
        ])
        .join(", ");

    const delegates = tables
        .map((table) => `    readonly ${camelCase(table.name)}: ${table.name}Delegate;`)
        .join("\n");

    const delegateClasses = tables
        .map(
            (table) => `
export class ${table.name}Delegate {
    constructor(private readonly binding: IrisDbBinding) {}

    async findMany<A extends ${table.name}FindManyArgs>(
        args?: A,
    ): Promise<${table.name}FindManyResult<A>> {
        const { source, parameters } = synthesizeFindMany("${table.name}", args);
        const rows = await this.binding.query(source, parameters);
        return rows as ${table.name}FindManyResult<A>;
    }

    async findUnique<A extends ${table.name}FindUniqueArgs>(
        args: A,
    ): Promise<${table.name}FindUniqueResult<A>> {
        const { source, parameters } = synthesizeFindUnique("${table.name}", args);
        const rows = (await this.binding.query(source, parameters)) as unknown[];
        return (rows[0] ?? null) as ${table.name}FindUniqueResult<A>;
    }

    async create<A extends ${table.name}CreateArgs>(
        args: A,
    ): Promise<${table.name}CreateResult<A>> {
        const { source, parameters } = synthesizeCreate("${table.name}", args as { data: Record<string, unknown>; select?: Record<string, unknown> });
        const row = await this.binding.query(source, parameters);
        return row as ${table.name}CreateResult<A>;
    }
}`,
        )
        .join("\n");

    return `import type { IrisDbBinding } from "@yydb/iris/types";
import { GeneratedMacros } from "./macros.js";
import { synthesizeCreate, synthesizeFindMany, synthesizeFindUnique } from "./synthesize.js";
import type {
    ${inputImports}
} from "./inputs.js";

${delegateClasses}

export class DbClient {
${delegates}
    readonly $macros: GeneratedMacros;
    private readonly binding: IrisDbBinding;

    constructor(binding: IrisDbBinding) {
        this.binding = binding;
        this.$macros = new GeneratedMacros(binding);
${tables
    .map((table) => {
        const lower = camelCase(table.name);
        return `        this.${lower} = new ${table.name}Delegate(binding);`;
    })
    .join("\n")}
    }

    /** Direct VOS DML. Caller declares result type; Rust validates semantics at runtime. */
    async $query<T = unknown>(
        source: string,
        parameters?: Readonly<Record<string, unknown>>,
    ): Promise<T> {
        return (await this.binding.query(source, parameters)) as T;
    }

    /** Direct VOS DDL. Success maps VOS unit to void. */
    async $execute(
        source: string,
        parameters?: Readonly<Record<string, unknown>>,
    ): Promise<void> {
        await this.binding.execute(source, parameters);
    }

    async $close(): Promise<void> {
        await this.binding.close();
    }
}

/** Construct a client from an injected binding (Node/Web host wiring stays outside generated types). */
export function createClient(binding: IrisDbBinding): DbClient {
    return new DbClient(binding);
}
`;
}

function emitIndex(): string {
    return `export { DbClient, createClient } from "./db.js";
export type * from "./models.js";
export type * from "./inputs.js";
export type * from "./references.js";
export { GeneratedMacros } from "./macros.js";
export { IRIS_SCHEMA_FINGERPRINT, IRIS_GENERATOR_VERSION } from "./metadata.js";
`;
}

async function loadSynthesizeSource(): Promise<string> {
    const here = dirname(fileURLToPath(import.meta.url));
    return await readFile(join(here, "synthesize-vos.ts"), "utf8");
}

/** Emit generated TypeScript client files into `outDir/generated/`. */
export async function emitTypescriptClient(options: EmitTypescriptOptions): Promise<EmitTypescriptResult> {
    const { mkdir, writeFile } = await import("node:fs/promises");
    const { join } = await import("node:path");
    const root = join(options.outDir, "generated");
    await mkdir(root, { recursive: true });

    const macros = options.introspection.macros ?? [];
    const synthesizeSource = await loadSynthesizeSource();

    const files = {
        "metadata.ts": emitMetadata(options.introspection),
        "references.ts": emitReferences(options.introspection.tables),
        "models.ts": emitModels(options.introspection.tables),
        "inputs.ts": emitInputs(options.introspection.tables),
        "macros.ts": emitMacros(macros),
        "synthesize.ts": synthesizeSource,
        "db.ts": emitDb(options.introspection.tables),
        "index.ts": emitIndex(),
    };

    const written: string[] = [];
    for (const [name, content] of Object.entries(files)) {
        const path = join(root, name);
        await writeFile(path, content, "utf8");
        written.push(path);
    }
    return { files: written };
}
