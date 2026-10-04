import type { OperationRequest, ResultEnvelope } from "../types/contract.ts";
import type { ExecutionRow } from "../types/execution-result.ts";
import type { OperationExecutor } from "../types/operation-executor.ts";
import {
    applyD1ReturningProjection,
    bindD1Parameters,
    resolveD1PlanForRequest,
    type D1PhysicalPlan,
    type D1PlanRegistry,
} from "./d1-plan.ts";
import type { IrisD1Database } from "./types.ts";

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
    d1: IrisD1Database,
    plan: D1PhysicalPlan,
    wireToTs: Readonly<Record<string, string>>,
    parameters?: Readonly<Record<string, unknown>>,
): Promise<ResultEnvelope<readonly ExecutionRow[]>> {
    try {
        const bindValues = bindD1Parameters(plan, parameters);
        const statement = d1.prepare(plan.sql).bind(...bindValues);
        const { results } = await statement.all<Record<string, unknown>>();
        const rows = (results ?? []).map((row) => wireRowToAuthorRow(wireToTs, row));
        return { ok: true, value: rows };
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        return notWired("IRIS-D1-EXEC-FAILED", message);
    }
}

async function executeWriteReturningPlan(
    d1: IrisD1Database,
    plan: D1PhysicalPlan,
    wireToTs: Readonly<Record<string, string>>,
    parameters?: Readonly<Record<string, unknown>>,
): Promise<ResultEnvelope<readonly ExecutionRow[]>> {
    try {
        const bindValues = bindD1Parameters(plan, parameters);
        const sql = applyD1ReturningProjection(plan.sql, parameters?.select_cols);
        const statement = d1.prepare(sql).bind(...bindValues);
        const { results } = await statement.all<Record<string, unknown>>();
        const rows = (results ?? []).map((row) => wireRowToAuthorRow(wireToTs, row));
        return { ok: true, value: rows };
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        return notWired("IRIS-D1-EXEC-FAILED", message);
    }
}

async function executeWritePlan(
    d1: IrisD1Database,
    plan: D1PhysicalPlan,
    parameters?: Readonly<Record<string, unknown>>,
): Promise<ResultEnvelope<void>> {
    try {
        const bindValues = bindD1Parameters(plan, parameters);
        const result = await d1.prepare(plan.sql).bind(...bindValues).run();
        if (!result.success) {
            return notWired("IRIS-D1-EXEC-FAILED", "D1 write returned success=false");
        }
        return { ok: true, value: undefined };
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        return notWired("IRIS-D1-EXEC-FAILED", message);
    }
}

/**
 * D1 operation executor driven by build-time plan artifacts.
 *
 * Read and `write-returning` plans run on `execute`. Plain `write` plans run on `executeUnit`.
 */
export function createD1OperationExecutor(
    d1: IrisD1Database,
    plans: D1PlanRegistry = {},
    wireNamesByEntity: Readonly<Record<string, Readonly<Record<string, string>>>> = {},
): OperationExecutor {
    return {
        async execute(request: OperationRequest): Promise<ResultEnvelope<readonly ExecutionRow[]>> {
            const plan = resolveD1PlanForRequest(plans, request);
            if (!plan) {
                return notWired(
                    "IRIS-D1-PLAN-MISSING",
                    `no D1 plan for operationId ${request.identity.operationId}`,
                );
            }
            const entity = entityFromOperationId(request.identity.operationId);
            const wireToTs = entity ? wireNamesByEntity[entity] ?? {} : {};
            if (plan.mode === "read") {
                return executeReadPlan(d1, plan, wireToTs, request.parameters);
            }
            if (plan.mode === "write-returning") {
                return executeWriteReturningPlan(d1, plan, wireToTs, request.parameters);
            }
            return notWired(
                "IRIS-D1-PLAN-INVALID",
                `D1 plan for ${request.identity.operationId} is not readable (mode=${plan.mode})`,
            );
        },
        async executeUnit(request: OperationRequest): Promise<ResultEnvelope<void>> {
            const plan = resolveD1PlanForRequest(plans, request);
            if (!plan) {
                return notWired(
                    "IRIS-D1-PLAN-MISSING",
                    `no D1 plan for operationId ${request.identity.operationId}`,
                );
            }
            if (plan.mode !== "write") {
                return notWired(
                    "IRIS-D1-PLAN-INVALID",
                    `D1 plan for ${request.identity.operationId} is not a plain write`,
                );
            }
            return executeWritePlan(d1, plan, request.parameters);
        },
        async close() {},
    };
}

/** @deprecated Use `createD1OperationExecutor`. */
export const createD1ReadOperationExecutor = createD1OperationExecutor;
