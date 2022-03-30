/** Runtime VOS synthesis for generated delegates (use-site reference paths, not include/relation). */

export type VosParameters = Record<string, unknown>;

export type VosSynthesis = {
    source: string;
    parameters: VosParameters;
};

export type FindManyArgs = {
    where?: Record<string, unknown>;
    select?: Record<string, unknown>;
    take?: number;
};

export type FindUniqueArgs = {
    where: Record<string, unknown>;
    select?: Record<string, unknown>;
};

const FILTER_OPS = new Set(["eq", "not", "gt", "gte", "lt", "lte", "contains", "startsWith", "endsWith"]);

function isPlainObject(value: unknown): value is Record<string, unknown> {
    return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isFilterOperatorObject(value: Record<string, unknown>): boolean {
    const keys = Object.keys(value);
    return keys.length > 0 && keys.every((key) => FILTER_OPS.has(key));
}

function nextParam(parameters: VosParameters, hint: string, value: unknown): string {
    const base = hint.replace(/[^a-zA-Z0-9_]/g, "_");
    let name = base;
    let index = 0;
    while (Object.prototype.hasOwnProperty.call(parameters, name)) {
        index += 1;
        name = `${base}_${index}`;
    }
    parameters[name] = value;
    return name;
}

function pathExpr(root: string, path: readonly string[]): string {
    return path.length === 0 ? root : `${root}.${path.join(".")}`;
}

/** Compile one leaf filter into a VOS predicate on `root` (e.g. `x.author.user_name != $p`). */
function compileLeafPredicate(
    root: string,
    path: readonly string[],
    value: unknown,
    parameters: VosParameters,
): string {
    const target = pathExpr(root, path);
    const hint = `p_${path.join("_") || "value"}`;

    if (value === true && path.length > 0) {
        return target;
    }
    if (value === false && path.length > 0) {
        return `!${target}`;
    }

    if (isPlainObject(value) && isFilterOperatorObject(value)) {
        const clauses: string[] = [];
        if ("eq" in value) {
            const param = nextParam(parameters, hint, value.eq);
            clauses.push(`${target} == $${param}`);
        }
        if ("not" in value) {
            const param = nextParam(parameters, `${hint}_not`, value.not);
            clauses.push(`${target} != $${param}`);
        }
        if ("gt" in value) {
            const param = nextParam(parameters, `${hint}_gt`, value.gt);
            clauses.push(`${target} > $${param}`);
        }
        if ("gte" in value) {
            const param = nextParam(parameters, `${hint}_gte`, value.gte);
            clauses.push(`${target} >= $${param}`);
        }
        if ("lt" in value) {
            const param = nextParam(parameters, `${hint}_lt`, value.lt);
            clauses.push(`${target} < $${param}`);
        }
        if ("lte" in value) {
            const param = nextParam(parameters, `${hint}_lte`, value.lte);
            clauses.push(`${target} <= $${param}`);
        }
        if ("contains" in value) {
            const param = nextParam(parameters, `${hint}_contains`, value.contains);
            clauses.push(`${target}.contains($${param})`);
        }
        if ("startsWith" in value) {
            const param = nextParam(parameters, `${hint}_starts`, value.startsWith);
            clauses.push(`${target}.starts_with($${param})`);
        }
        if ("endsWith" in value) {
            const param = nextParam(parameters, `${hint}_ends`, value.endsWith);
            clauses.push(`${target}.ends_with($${param})`);
        }
        return clauses.length === 1 ? clauses[0]! : `(${clauses.join(" && ")})`;
    }

    const param = nextParam(parameters, hint, value);
    return `${target} == $${param}`;
}

/**
 * Walk a nested where object into VOS predicates.
 * Nested plain objects that are not filter ops are treated as reference path navigation
 * (e.g. `{ author: { user_name: { not: "" } } }` → `x.author.user_name != $p`).
 */
export function compileWherePredicates(
    where: Record<string, unknown>,
    parameters: VosParameters,
    root = "x",
    path: readonly string[] = [],
): string[] {
    const predicates: string[] = [];
    for (const [key, value] of Object.entries(where)) {
        if (value === undefined) {
            continue;
        }
        const nextPath = [...path, key];
        if (isPlainObject(value) && !isFilterOperatorObject(value)) {
            predicates.push(...compileWherePredicates(value, parameters, root, nextPath));
            continue;
        }
        predicates.push(compileLeafPredicate(root, nextPath, value, parameters));
    }
    return predicates;
}

/**
 * Compile nested select into a VOS projection expression rooted at `root`.
 * Reference fields with nested objects become dereference projections:
 * `{ author: { userId: true } }` → `x.{ author: x.author.{ userId } }`
 */
export function compileSelectProjection(select: Record<string, unknown>, root = "x"): string {
    const parts: string[] = [];
    for (const [key, value] of Object.entries(select)) {
        if (value === true) {
            parts.push(key);
            continue;
        }
        if (isPlainObject(value)) {
            const nested = compileSelectProjection(value, `${root}.${key}`);
            parts.push(`${key}: ${nested}`);
        }
    }
    return `${root}.{ ${parts.join(", ")} }`;
}

function appendFilter(entity: string, where: Record<string, unknown> | undefined, parameters: VosParameters): string {
    if (!where || Object.keys(where).length === 0) {
        return entity;
    }
    const predicates = compileWherePredicates(where, parameters);
    if (predicates.length === 0) {
        return entity;
    }
    const body = predicates.length === 1 ? predicates[0]! : predicates.join(" && ");
    return `${entity}.filter(x => ${body})`;
}

function appendMap(pipeline: string, select: Record<string, unknown> | undefined): string {
    if (!select || Object.keys(select).length === 0) {
        return pipeline;
    }
    return `${pipeline}.map(x => ${compileSelectProjection(select, "x")})`;
}

/** Synthesize VOS for findMany — where paths + select projections are use-site IR for the planner. */
export function synthesizeFindMany(entity: string, args?: FindManyArgs): VosSynthesis {
    const parameters: VosParameters = {};
    let pipeline = appendFilter(entity, args?.where, parameters);
    pipeline = appendMap(pipeline, args?.select);
    if (args?.take != null) {
        parameters.take = args.take;
        pipeline += ".take($take)";
    }
    pipeline += ".collect()";
    return { source: pipeline, parameters };
}

/** Synthesize VOS for findUnique — same use-site rules as findMany. */
export function synthesizeFindUnique(entity: string, args: FindUniqueArgs): VosSynthesis {
    const parameters: VosParameters = {};
    let pipeline = appendFilter(entity, args.where, parameters);
    pipeline = appendMap(pipeline, args.select);
    pipeline += ".collect()";
    return { source: pipeline, parameters };
}

/** Flatten `&T` write inputs (branded id | { pk } | Reference) to a scalar bind value. */
export function flattenRefInput(value: unknown): unknown {
    if (isPlainObject(value)) {
        for (const [key, nested] of Object.entries(value)) {
            if (key.startsWith("__")) {
                continue;
            }
            return nested;
        }
    }
    return value;
}

export type CreateArgs = {
    data: Record<string, unknown>;
    select?: Record<string, unknown>;
};

/** Synthesize `Entity::insert({ field: $p, ... })` with bound parameters (no placeholders). */
export function synthesizeCreate(entity: string, args: CreateArgs): VosSynthesis {
    const parameters: VosParameters = {};
    const fields: string[] = [];
    for (const [key, value] of Object.entries(args.data)) {
        if (value === undefined) {
            continue;
        }
        const param = nextParam(parameters, `data_${key}`, flattenRefInput(value));
        fields.push(`${key}: $${param}`);
    }
    let source = `${entity}::insert({ ${fields.join(", ")} })`;
    if (args.select && Object.keys(args.select).length > 0) {
        source = `(${source}).map(x => ${compileSelectProjection(args.select, "x")})`;
    }
    return { source, parameters };
}

/** Synthesize a schema macro call `name($arg0, $arg1, ...)` with positional binds. */
export function synthesizeMacroCall(name: string, args: readonly unknown[]): VosSynthesis {
    const parameters: VosParameters = {};
    const placeholders = args.map((value, index) => {
        const param = nextParam(parameters, `arg${index}`, flattenRefInput(value));
        return `$${param}`;
    });
    return {
        source: `${name}(${placeholders.join(", ")})`,
        parameters,
    };
}
