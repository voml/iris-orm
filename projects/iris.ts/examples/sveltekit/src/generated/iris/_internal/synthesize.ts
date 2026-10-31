/** Runtime VOS synthesis for generated delegates (use-site reference paths, not include/relation). */
import type { OperationRequest, ResultEnvelope } from "./contract-types.js";
import type { IrisGeneratedError } from "../errors.js";
import { IRIS_SCHEMA_FINGERPRINT } from "../metadata.js";
// Pre-computed per-entity maps inlined at codegen time — synthesize.ts is
// self-contained for wire-name + reference-target lookup and no longer pulls
// IRIS_FIELD_WIRE_NAMES / IRIS_REFERENCE_TARGETS from metadata.ts. Those
// constants remain in metadata.ts for node/browser/cloudflare entry points.
const SYNTHESIZE_WIRE_NAMES: Record<string, Readonly<Record<string, string>>> = {
    User: { userId: "user_id", userName: "user_name", active: "active" },
    Post: { postId: "post_id", author: "author", title: "title", published: "published" },
};
const SYNTHESIZE_REF_TARGETS: Record<string, Readonly<Record<string, string>>> = {
    User: {  },
    Post: { author: "User" },
};
/**
 * Field → operator allow-list declared by each field's faithful VOS label at
 * codegen time. The runtime lowers filters strictly inside this surface, so
 * an untyped caller cannot widen a `uuid` field to `contains` the way the
 * old carrier approximation let a plain object pretend to be any filter.
 */
const SYNTHESIZE_FILTER_OPS: Record<string, Readonly<Record<string, readonly string[]>>> = {
    User: { userId: ["eq", "not"], userName: ["eq", "not", "contains", "startsWith", "endsWith"], active: ["eq", "not"] },
    Post: { postId: ["eq", "not"], title: ["eq", "not", "contains", "startsWith", "endsWith"], published: ["eq", "not"] },
};
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
const FILTER_OPS = new Set([
    "eq",
    "not",
    "gt",
    "gte",
    "lt",
    "lte",
    "contains",
    "startsWith",
    "endsWith",
    "isNull",
    "isNotNull",
]);
/** A record literal only: `Uint8Array` / `ArrayBuffer` carriers and class instances must not look like `{ pk }`. */
function isPlainObject(value: unknown): value is Record<string, unknown> {
    if (typeof value !== "object" || value === null || Array.isArray(value)) {
        return false;
    }
    const proto = Object.getPrototypeOf(value);
    return proto === Object.prototype || proto === null;
}
function isFilterOperatorObject(value: Record<string, unknown>): boolean {
    const keys = Object.keys(value);
    return keys.length > 0 && keys.every((key) => FILTER_OPS.has(key));
}
function wireName(entity: string, tsField: string): string {
    return SYNTHESIZE_WIRE_NAMES[entity]?.[tsField] ?? tsField;
}
function referenceTargetEntity(entity: string, tsField: string): string | undefined {
    return SYNTHESIZE_REF_TARGETS[entity]?.[tsField];
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
    entity: string,
    tsKey: string,
    value: unknown,
    parameters: VosParameters,
): string {
    const target = pathExpr(root, path);
    const hint = `p_${path.join("_") || "value"}`;
    const allowed = SYNTHESIZE_FILTER_OPS[entity]?.[tsKey];
    if (!allowed) {
        const error: IrisGeneratedError = {
            code: "IRIS-FILTER-FIELD-UNKNOWN",
            message: `filter field \`${entity}.${tsKey}\` declares no filter surface`,
        };
        throw error;
    }
    if (value === true && path.length > 0) {
        return target;
    }
    if (value === false && path.length > 0) {
        return `!${target}`;
    }
    if (isPlainObject(value) && isFilterOperatorObject(value)) {
        for (const operator of Object.keys(value)) {
            if (!allowed.includes(operator)) {
                const error: IrisGeneratedError = {
                    code: "IRIS-FILTER-OPERATOR-NOT-SUPPORTED",
                    message: `operator \`${operator}\` is outside the \`${entity}.${tsKey}\` filter surface [${allowed.join(", ")}]`,
                };
                throw error;
            }
        }
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
        if ("isNull" in value && value.isNull === true) {
            clauses.push(`${target} == null`);
        }
        if ("isNotNull" in value && value.isNotNull === true) {
            clauses.push(`${target} != null`);
        }
        return clauses.length === 1 ? clauses[0]! : `(${clauses.join(" && ")})`;
    }
    const param = nextParam(parameters, hint, value);
    return `${target} == $${param}`;
}
/**
 * Walk a nested where object into VOS predicates.
 * TypeScript author keys are lowered to VOS wire field names per entity.
 */
export function compileWherePredicates(
    entity: string,
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
        const wireKey = wireName(entity, key);
        if (key === "is" && isPlainObject(value)) {
            predicates.push(...compileWherePredicates(entity, value, parameters, root, path));
            continue;
        }
        if (isPlainObject(value) && !isFilterOperatorObject(value)) {
            const target = referenceTargetEntity(entity, key);
            const nextEntity = target ?? entity;
            predicates.push(...compileWherePredicates(nextEntity, value, parameters, root, [...path, wireKey]));
            continue;
        }
        predicates.push(compileLeafPredicate(root, [...path, wireKey], entity, key, value, parameters));
    }
    return predicates;
}
/**
 * Compile nested select into a VOS projection expression rooted at `root`.
 * TypeScript author keys are lowered to VOS wire field names per entity.
 */
/** Flat scalar wire columns selected with `{ field: true }` (sorted for plan metadata). */
export function compileSelectWireColumns(entity: string, select: Record<string, unknown>): string[] {
    const wires: string[] = [];
    for (const [key, value] of Object.entries(select)) {
        if (value === true) {
            wires.push(wireName(entity, key));
        }
    }
    wires.sort();
    return wires;
}
function appendSelectColsParameter(
    entity: string,
    select: Record<string, unknown> | undefined,
    parameters: VosParameters,
): void {
    if (!select || Object.keys(select).length === 0) {
        return;
    }
    const wires = compileSelectWireColumns(entity, select);
    if (wires.length > 0) {
        parameters.select_cols = wires.join(",");
    }
}
export function compileSelectProjection(
    entity: string,
    select: Record<string, unknown>,
    root = "x",
): string {
    const parts: string[] = [];
    for (const [key, value] of Object.entries(select)) {
        const wireKey = wireName(entity, key);
        if (value === true) {
            parts.push(wireKey);
            continue;
        }
        if (isPlainObject(value)) {
            const nestedSelect = isPlainObject(value.select) ? value.select : value;
            const target = referenceTargetEntity(entity, key);
            const nestedEntity = target ?? entity;
            const nested = compileSelectProjection(nestedEntity, nestedSelect, `${root}.${wireKey}`);
            parts.push(`${wireKey}: ${nested}`);
        }
    }
    return `${root}.{ ${parts.join(", ")} }`;
}
function appendFilter(entity: string, where: Record<string, unknown> | undefined, parameters: VosParameters): string {
    if (!where || Object.keys(where).length === 0) {
        return entity;
    }
    const predicates = compileWherePredicates(entity, where, parameters);
    if (predicates.length === 0) {
        return entity;
    }
    const body = predicates.length === 1 ? predicates[0]! : predicates.join(" && ");
    return `${entity}.filter(x => ${body})`;
}
function appendMap(entity: string, pipeline: string, select: Record<string, unknown> | undefined): string {
    if (!select || Object.keys(select).length === 0) {
        return pipeline;
    }
    return `${pipeline}.map(x => ${compileSelectProjection(entity, select, "x")})`;
}
/** Synthesize VOS for findMany — where paths + select projections are use-site IR for the planner. */
export function synthesizeFindMany(entity: string, args?: FindManyArgs): VosSynthesis {
    const parameters: VosParameters = {};
    let pipeline = appendFilter(entity, args?.where, parameters);
    pipeline = appendMap(entity, pipeline, args?.select);
    if (args?.take != null) {
        parameters.take = args.take;
        pipeline += ".take($take)";
    }
    pipeline += ".collect()";
    return { source: pipeline, parameters };
}
export function synthesizeFindFirst(entity: string, args?: FindManyArgs): VosSynthesis {
    return synthesizeFindMany(entity, { ...args, take: 1 });
}
/** Synthesize VOS for findUnique — same use-site rules as findMany. */
export function synthesizeFindUnique(entity: string, args: FindUniqueArgs): VosSynthesis {
    const parameters: VosParameters = {};
    let pipeline = appendFilter(entity, args.where, parameters);
    pipeline = appendMap(entity, pipeline, args.select);
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
export type DeleteArgs = {
    where: Record<string, unknown>;
};
export type UpdateArgs = {
    where: Record<string, unknown>;
    data: Record<string, unknown>;
    select?: Record<string, unknown>;
};
function compilePatchAssignments(
    entity: string,
    data: Record<string, unknown>,
    parameters: VosParameters,
): string[] {
    const assignments: string[] = [];
    for (const [key, value] of Object.entries(data)) {
        if (value === undefined) {
            continue;
        }
        const wireKey = wireName(entity, key);
        if (isPlainObject(value) && "set" in value) {
            const param = nextParam(parameters, `patch_${wireKey}`, flattenRefInput(value.set));
            assignments.push(`${wireKey}: $${param}`);
        } else if (isPlainObject(value) && "setNull" in value && value.setNull === true) {
            assignments.push(`${wireKey}: null`);
        }
    }
    return assignments;
}
/** Synthesize `Entity.filter(...).patch({ ... })` with bound where and patch parameters. */
export function synthesizeUpdate(entity: string, args: UpdateArgs): VosSynthesis {
    const parameters: VosParameters = {};
    const predicates = compileWherePredicates(entity, args.where, parameters);
    const assignments = compilePatchAssignments(entity, args.data, parameters);
    const body = predicates.length === 1 ? predicates[0]! : predicates.join(" && ");
    let source = `${entity}.filter(x => ${body}).patch({ ${assignments.join(", ")} })`;
    if (args.select && Object.keys(args.select).length > 0) {
        appendSelectColsParameter(entity, args.select, parameters);
        source = `(${source}).map(x => ${compileSelectProjection(entity, args.select, "x")})`;
    }
    return { source, parameters };
}
/** Synthesize `Entity.filter(...).delete()` with bound where parameters. */
export function synthesizeDelete(entity: string, args: DeleteArgs): VosSynthesis {
    const parameters: VosParameters = {};
    let pipeline = appendFilter(entity, args.where, parameters);
    pipeline += ".delete()";
    return { source: pipeline, parameters };
}
/** Synthesize `Entity::insert({ field: $p, ... })` with bound parameters (no placeholders). */
export function synthesizeCreate(entity: string, args: CreateArgs): VosSynthesis {
    const parameters: VosParameters = {};
    const fields: string[] = [];
    for (const [key, value] of Object.entries(args.data)) {
        if (value === undefined) {
            continue;
        }
        const wireKey = wireName(entity, key);
        const param = nextParam(parameters, `data_${wireKey}`, flattenRefInput(value));
        fields.push(`${wireKey}: $${param}`);
    }
    let source = `${entity}::insert({ ${fields.join(", ")} })`;
    if (args.select && Object.keys(args.select).length > 0) {
        appendSelectColsParameter(entity, args.select, parameters);
        source = `(${source}).map(x => ${compileSelectProjection(entity, args.select, "x")})`;
    }
    return { source, parameters };
}
/** Synthesize a declared operation invocation `name($arg0, $arg1, ...)` with positional binds. */
export function synthesizeOperationCall(name: string, args: readonly unknown[]): VosSynthesis {
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
/** Build a declared operation request for the generated client primary ABI. */
export function buildDeclaredOperationRequest(operationId: string, synthesis: VosSynthesis): OperationRequest {
    return {
        identity: { operationId, contractFingerprint: IRIS_SCHEMA_FINGERPRINT },
        operation: { kind: "declared-vos", source: synthesis.source },
        parameters: synthesis.parameters,
    };
}
/** Throw a generated-client error from a failed result envelope. */
export function throwOperationEnvelopeError(envelope: Extract<ResultEnvelope<unknown>, { ok: false }>): never {
    const diagnostic = envelope.diagnostics[0];
    const error: IrisGeneratedError = {
        code: diagnostic?.code ?? "IRIS-EXEC-FAILED",
        message: diagnostic?.message ?? "iris operation failed",
    };
    throw error;
}
