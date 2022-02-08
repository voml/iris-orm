import type { SchemaFieldModel, SchemaTableModel } from "../types/schema-introspection.ts";
import { modelFieldType, scalarFilterType } from "./emit-references.ts";

function filterOpsFor(scalarTs: string): string {
    return `| {
    eq?: ${scalarTs};
    not?: ${scalarTs};
    gt?: ${scalarTs};
    gte?: ${scalarTs};
    lt?: ${scalarTs};
    lte?: ${scalarTs};
    contains?: string;
    startsWith?: string;
    endsWith?: string;
}`;
}

/**
 * Emit inputs.ts with:
 * - nested reference where/select paths (not include/relation)
 * - argument-sensitive Selected<Entity> result types for nested &T projections
 */
export function emitInputs(tables: SchemaTableModel[]): string {
    const entityNames = tables.map((table) => table.name);
    const entityUnion = entityNames.map((name) => `"${name}"`).join(" | ") || "never";

    const entityMap = entityNames.map((name) => `    ${name}: ${name};`).join("\n");

    const header = `import type {
${entityNames.map((name) => `    ${name},`).join("\n")}
} from "./models.js";
import type {
${entityNames.flatMap((name) => [`    ${name}Id,`, `    ${name}RefInput,`]).join("\n")}
} from "./references.js";

type EntityByName = {
${entityMap}
};

type EntityName = ${entityUnion};

/** Nested select shape for a reference target (use-site dereference projection). */
export type SelectPathFor<E extends EntityName> = {
    [K in keyof EntityByName[E]]?:
        | boolean
        | (EntityByName[E][K] extends { readonly __irisRef: infer T extends EntityName }
              ? SelectPathFor<T>
              : never);
};

/** Nested where path for a reference target (use-site filter navigation). */
export type WherePathFor<E extends EntityName> = {
    [K in keyof EntityByName[E]]?:
        | WhereValueForField<EntityByName[E][K]>
        | (EntityByName[E][K] extends { readonly __irisRef: infer T extends EntityName }
              ? WherePathFor<T>
              : never);
};

type WhereValueForField<V> = V extends { readonly __irisRef: EntityName }
    ? never
    : V | FilterOps<V extends null | undefined ? NonNullable<V> : V>;

type FilterOps<T> = {
    eq?: T;
    not?: T;
    gt?: T;
    gte?: T;
    lt?: T;
    lte?: T;
    contains?: string;
    startsWith?: string;
    endsWith?: string;
};

/**
 * Resolve a select shape against an entity — nested object on &T becomes
 * SelectResultFor of the target; \`true\` on &T means full target entity.
 */
export type SelectResultFor<E extends EntityName, S> = [S] extends [undefined]
    ? EntityByName[E]
    : {
          [K in keyof S & keyof EntityByName[E] as S[K] extends true | object ? K : never]: S[K] extends true
              ? EntityByName[E][K] extends { readonly __irisRef: infer T extends EntityName }
                  ? EntityByName[T]
                  : EntityByName[E][K]
              : S[K] extends object
                ? EntityByName[E][K] extends { readonly __irisRef: infer T extends EntityName }
                    ? SelectResultFor<T, S[K]>
                    : never
                : never;
      };
`;

    const blocks = tables.map((table) => emitEntityInputs(table, new Set(entityNames)));
    return `${header}\n${blocks.join("\n")}\n`;
}

function emitEntityInputs(table: SchemaTableModel, entityNames: Set<string>): string {
    const primaryFields = table.fields.filter((field) => field.primary);
    const uniqueWhere =
        primaryFields.length > 0
            ? primaryFields.map((field) => `    ${field.name}?: ${table.name}Id;`).join("\n")
            : `    id?: ${table.name}Id;`;

    const whereFields = table.fields
        .map((field) => emitWhereField(table, field, entityNames))
        .filter(Boolean)
        .join("\n");

    const selectFields = table.fields
        .map((field) => emitSelectField(field, entityNames))
        .filter(Boolean)
        .join("\n");

    const createFields = table.fields
        .map((field) => emitCreateField(table, field, entityNames))
        .filter(Boolean)
        .join("\n");

    return `
export type ${table.name}Select = {
${selectFields}
};

export type ${table.name}WhereInput = {
${whereFields}
};

export type ${table.name}WhereUniqueInput = {
${uniqueWhere}
};

export type ${table.name}CreateInput = {
${createFields}
};

export type ${table.name}UpdateInput = {
    [K in keyof ${table.name}CreateInput]?: ${table.name}CreateInput[K];
};

export type ${table.name}FindManyArgs = {
    where?: ${table.name}WhereInput;
    select?: ${table.name}Select;
    take?: number;
};

export type ${table.name}FindUniqueArgs = {
    where: ${table.name}WhereUniqueInput;
    select?: ${table.name}Select;
};

export type ${table.name}CreateArgs = {
    data: ${table.name}CreateInput;
    select?: ${table.name}Select;
};

export type ${table.name}FindManyResult<A extends ${table.name}FindManyArgs> = Array<
    SelectResultFor<"${table.name}", A["select"]>
>;

export type ${table.name}FindUniqueResult<A extends ${table.name}FindUniqueArgs> =
    SelectResultFor<"${table.name}", A["select"]> | null;

export type ${table.name}CreateResult<A extends ${table.name}CreateArgs> = SelectResultFor<
    "${table.name}",
    A["select"]
>;
`;
}

function emitWhereField(table: SchemaTableModel, field: SchemaFieldModel, entityNames: Set<string>): string {
    if (field.referenceTarget && entityNames.has(field.referenceTarget)) {
        return `    ${field.name}?: WherePathFor<"${field.referenceTarget}">;`;
    }
    if (field.referenceTarget) {
        return "";
    }
    const scalar = scalarFilterType(table, field);
    return `    ${field.name}?: ${scalar} ${filterOpsFor(scalar)};`;
}

function emitSelectField(field: SchemaFieldModel, entityNames: Set<string>): string {
    if (field.referenceTarget && entityNames.has(field.referenceTarget)) {
        return `    ${field.name}?: boolean | SelectPathFor<"${field.referenceTarget}">;`;
    }
    if (field.referenceTarget) {
        return "";
    }
    return `    ${field.name}?: boolean;`;
}

function emitCreateField(table: SchemaTableModel, field: SchemaFieldModel, entityNames: Set<string>): string {
    if (field.referenceTarget) {
        if (!entityNames.has(field.referenceTarget)) {
            return "";
        }
        const req = field.optional ? "?" : "";
        return `    ${field.name}${req}: ${field.referenceTarget}RefInput;`;
    }
    if (field.optional) {
        return `    ${field.name}?: ${modelFieldType(table, field)};`;
    }
    return `    ${field.name}: ${modelFieldType(table, field)};`;
}
