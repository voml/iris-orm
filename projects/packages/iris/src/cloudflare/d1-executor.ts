import type { OperationRequest, ResultEnvelope } from "../types/contract.ts";
import type { ExecutionRow } from "../types/execution-result.ts";
import type { OperationExecutor } from "../types/operation-executor.ts";
import type { D1PlanRegistry, D1PhysicalPlan } from "./d1-plan.ts";
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
): Promise<ResultEnvelope<readonly ExecutionRow[]>> {
    try {
        const { results } = await d1.prepare(plan.sql).bind().all<Record<string, unknown>>();
        const rows = (results ?? []).map((row) => wireRowToAuthorRow(wireToTs, row));
        return { ok: true, value: rows };
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        return notWired("IRIS-D1-EXEC-FAILED", message);
    }
}

/**
 * Read-only D1 operation executor driven by build-time plan artifacts.
 *
 * Mutation operations return structured not-wired diagnostics until Phase 4.
 */
export function createD1ReadOperationExecutor(
    d1: IrisD1Database,
    plans: D1PlanRegistry = {},
    wireNamesByEntity: Readonly<Record<string, Readonly<Record<string, string>>>> = {},
): OperationExecutor {
    return {
        async execute(request: OperationRequest): Promise<ResultEnvelope<readonly ExecutionRow[]>> {
            const plan = plans[request.identity.operationId];
            if (!plan) {
                return notWired(
                    "IRIS-D1-PLAN-MISSING",
                    `no D1 read plan for operationId ${request.identity.operationId}`,
                );
            }
            if (plan.mode !== "read") {
                return notWired("IRIS-D1-PLAN-INVALID", `D1 plan for ${request.identity.operationId} is not read-only`);
            }
            const entity = entityFromOperationId(request.identity.operationId);
            const wireToTs = entity ? wireNamesByEntity[entity] ?? {} : {};
            return executeReadPlan(d1, plan, wireToTs);
        },
        async executeUnit(_request: OperationRequest): Promise<ResultEnvelope<void>> {
            return notWired(
                "IRIS-D1-WRITE-NOT-WIRED",
                "D1 mutation path is not available in read-only Cloudflare profile yet",
            );
        },
        async close() {},
    };
}
