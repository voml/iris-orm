import type { OperationRequest, ResultEnvelope } from "../types/contract.ts";
import type { ExecutionRow } from "../types/execution-result.ts";
import type { OperationExecutor } from "../types/operation-executor.ts";
import {
    applyD1ReturningProjection,
    bindD1Parameters,
    resolveD1PlanForRequest,
    type D1PhysicalPlan,
    type D1PlanRegistry,
} from "../cloudflare/d1-plan.ts";
import type { IrisSqliteDatabase } from "./types.ts";

function notWired(code: string, message: string): ResultEnvelope<never> {
    return {
        ok: false,
        diagnostics: [{ code, message, severity: "error" }],
    };
}

function entityFromOperationId(operationId: string): string | null {
    const dot = operationId.indexOf(".");
    if (dot <= 0) {
        return null;
    }
    return operationId.slice(0, dot);
}

function wireRowToAuthorRow(
    wireToTs: Readonly<Record<string, string>>,
    row: Record<string, unknown>,
): ExecutionRow {
    const mapped: ExecutionRow = {};
    for (const [tsName, wireName] of Object.entries(wireToTs)) {
        if (wireName in row) {
            const value = row[wireName];
            mapped[tsName] =
                typeof value === "string" || typeof value === "number" || typeof value === "boolean" || value === null
                    ? value
                    : value == null
                      ? null
                      : String(value);
        }
    }
    return mapped;
}

async function executeReadPlan(
    sqlite: IrisSqliteDatabase,
    plan: D1PhysicalPlan,
    wireToTs: Readonly<Record<string, string>>,
    parameters?: Readonly<Record<string, unknown>>,
): Promise<ResultEnvelope<readonly ExecutionRow[]>> {
    try {
        const bindValues = bindD1Parameters(plan, parameters);
        const statement = sqlite.prepare(plan.sql).bind(...bindValues);
        const { results } = await statement.all<Record<string, unknown>>();
        const rows = (results ?? []).map((row) => wireRowToAuthorRow(wireToTs, row));
        return { ok: true, value: rows };
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        return notWired("IRIS-SQLITE-EXEC-FAILED", message);
    }
}

async function executeWriteReturningPlan(
    sqlite: IrisSqliteDatabase,
    plan: D1PhysicalPlan,
    wireToTs: Readonly<Record<string, string>>,
    parameters?: Readonly<Record<string, unknown>>,
): Promise<ResultEnvelope<readonly ExecutionRow[]>> {
    try {
        const bindValues = bindD1Parameters(plan, parameters);
        const sql = applyD1ReturningProjection(plan.sql, parameters?.select_cols);
        const statement = sqlite.prepare(sql).bind(...bindValues);
        const { results } = await statement.all<Record<string, unknown>>();
        const rows = (results ?? []).map((row) => wireRowToAuthorRow(wireToTs, row));
        return { ok: true, value: rows };
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        return notWired("IRIS-SQLITE-EXEC-FAILED", message);
    }
}

async function executeWritePlan(
    sqlite: IrisSqliteDatabase,
    plan: D1PhysicalPlan,
    parameters?: Readonly<Record<string, unknown>>,
): Promise<ResultEnvelope<void>> {
    try {
        const bindValues = bindD1Parameters(plan, parameters);
        const result = await sqlite.prepare(plan.sql).bind(...bindValues).run();
        if (!result.success) {
            return notWired("IRIS-SQLITE-EXEC-FAILED", "SQLite write returned success=false");
        }
        return { ok: true, value: undefined };
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        return notWired("IRIS-SQLITE-EXEC-FAILED", message);
    }
}

/**
 * SQLite operation executor driven by build-time physical plan artifacts.
 *
 * Shared by Cloudflare D1 and other SQLite-shaped foreign adapters.
 */
export function createSqlitePlanOperationExecutor(
    sqlite: IrisSqliteDatabase,
    plans: D1PlanRegistry = {},
    wireNamesByEntity: Readonly<Record<string, Readonly<Record<string, string>>>> = {},
): OperationExecutor {
    return {
        async execute(request: OperationRequest): Promise<ResultEnvelope<readonly ExecutionRow[]>> {
            const plan = resolveD1PlanForRequest(plans, request);
            if (!plan) {
                return notWired(
                    "IRIS-SQLITE-PLAN-MISSING",
                    `no SQLite plan for operationId ${request.identity.operationId}`,
                );
            }
            const entity = entityFromOperationId(request.identity.operationId);
            const wireToTs = entity ? wireNamesByEntity[entity] ?? {} : {};
            if (plan.mode === "read") {
                return executeReadPlan(sqlite, plan, wireToTs, request.parameters);
            }
            if (plan.mode === "write-returning") {
                return executeWriteReturningPlan(sqlite, plan, wireToTs, request.parameters);
            }
            return notWired(
                "IRIS-SQLITE-PLAN-INVALID",
                `SQLite plan for ${request.identity.operationId} is not readable (mode=${plan.mode})`,
            );
        },
        async executeUnit(request: OperationRequest): Promise<ResultEnvelope<void>> {
            const plan = resolveD1PlanForRequest(plans, request);
            if (!plan) {
                return notWired(
                    "IRIS-SQLITE-PLAN-MISSING",
                    `no SQLite plan for operationId ${request.identity.operationId}`,
                );
            }
            if (plan.mode !== "write") {
                return notWired(
                    "IRIS-SQLITE-PLAN-INVALID",
                    `SQLite plan for ${request.identity.operationId} is not a plain write`,
                );
            }
            return executeWritePlan(sqlite, plan, request.parameters);
        },
        async close() {},
    };
}
