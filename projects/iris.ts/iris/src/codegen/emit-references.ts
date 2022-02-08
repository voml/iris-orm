import type { SchemaFieldModel, SchemaTableModel } from "../types/schema-introspection.ts";

function primaryKeyField(table: SchemaTableModel) {
    return table.fields.find((field) => field.primary) ?? table.fields[0];
}

/** Emit branded primary keys and reference input types (no circular model imports). */
export function emitReferences(tables: SchemaTableModel[]): string {
    const blocks = tables.map((table) => {
        const pk = primaryKeyField(table);
        const pkName = pk?.name ?? "id";
        const idType = `${table.name}Id`;
        const refType = `${table.name}Reference`;
        const refInput = `${table.name}RefInput`;

        return `declare const __iris${table.name}IdBrand: unique symbol;

/** Branded primary key for ${table.name}. */
export type ${idType} = ${scalarBaseType(pk?.vosType ?? "utf8")} & {
    readonly [__iris${table.name}IdBrand]: true;
};

/** Stored reference identity for &${table.name} (not an ORM relation object). */
export type ${refType} = {
    readonly __irisRef: "${table.name}";
    ${pkName}: ${idType};
};

/** Values accepted when writing &${table.name}. */
export type ${refInput} =
    | ${idType}
    | { ${pkName}: ${idType} }
    | ${refType};`;
    });

    return `${blocks.join("\n\n")}\n`;
}

function scalarBaseType(vosType: string): string {
    switch (vosType.replace("?", "").replace(/^&/, "")) {
        case "bool":
            return "boolean";
        case "utf8":
        case "uuid":
        case "decimal":
        case "datetime":
            return "string";
        case "i8":
        case "i16":
        case "i32":
        case "i64":
        case "u8":
        case "u16":
        case "u32":
        case "u64":
        case "f32":
        case "f64":
            return "number";
        default:
            return "string";
    }
}

/** Resolve TS field type for model surfaces. */
export function modelFieldType(table: SchemaTableModel, field: SchemaFieldModel): string {
    if (field.referenceTarget) {
        const ref = `${field.referenceTarget}Reference`;
        return field.optional ? `${ref} | null` : ref;
    }
    if (field.primary) {
        return `${table.name}Id`;
    }
    const base = scalarBaseType(field.vosType);
    return field.optional ? `${base} | null` : base;
}

/** Scalar TS type without nullability (for filter operators). */
export function scalarFilterType(table: SchemaTableModel, field: SchemaFieldModel): string {
    if (field.primary) {
        return `${table.name}Id`;
    }
    return scalarBaseType(field.vosType);
}

/** Collect reference targets declared in schema tables. */
export function referenceTargets(tables: SchemaTableModel[]): Set<string> {
    const targets = new Set<string>();
    for (const table of tables) {
        for (const field of table.fields) {
            if (field.referenceTarget) {
                targets.add(field.referenceTarget);
            }
        }
    }
    return targets;
}
