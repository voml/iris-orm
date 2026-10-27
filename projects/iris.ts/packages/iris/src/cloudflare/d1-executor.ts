/**
 * Cloudflare D1 `OperationExecutor` — host bridge on top of the Rust WASM
 * `prepare_d1_operation` / `decode_d1_result` ABI.
 *
 * Flow per call:
 *   1. resolve build-time `D1PhysicalPlan` from the request identity
 *   2. lift it into a full `D1Request` envelope (runtime params → WireValue)
 *   3. `@yydb/iris-language/cloudflare/d1-wasm-bridge` — WASM prepare,
 *      D1 `.prepare().bind().run()/.all()`, WASM decode
 *   4. canonical VOS Value rows → JS carrier rows for the Iris runtime
 *
 * This replaces the previous runtime-throw stub (P0 2026-10-27).  See
 * `packages/iris-language/src/cloudflare/d1-wasm-bridge.ts` for the WASM
 * round-trip contract.
 */

import type { OperationRequest, ResultEnvelope } from "../types/contract.ts";
import type { ExecutionRow } from "../types/execution-result.ts";
import type { OperationExecutor } from "../types/operation-executor.ts";

import {
    applyD1ReturningProjection,
    resolveD1PlanForRequest,
    type D1PhysicalPlan,
    type D1PlanRegistry,
} from "./d1-plan.ts";

import {
    canonicalRowToCarrierRow,
    executeD1Request,
    jsValueToWireValue,
    type CanonicalResult,
    type D1ExecutionMode,
    type D1HostDb,
    type D1Request,
    type ReturningProjection,
    type WireValue,
} from "@yydb/iris-language/cloudflare/d1-wasm-bridge";

// ---------------------------------------------------------------------------
// D1PhysicalPlan → D1Request lifting
// ---------------------------------------------------------------------------

/** Map from the old build-time plan mode to the Rust adapter D1ExecutionMode. */
function planModeToD1Mode(planMode: D1PhysicalPlan["mode"]): D1ExecutionMode {
    switch (planMode) {
        case "read":
            return "read";
        case "write":
            return "write";
        case "write-returning":
            return "write-returning";
    }
}

/** Build a `ReturningProjection` from runtime parameters (only relevant for write-returning). */
function buildReturningProjection(
    plan: D1PhysicalPlan,
    parameters?: Readonly<Record<string, unknown>>,
): ReturningProjection {
    if (plan.mode !== "write-returning") {
        return { kind: "none" };
    }
    const selectCols = parameters?.select_cols;
    if (typeof selectCols === "string") {
        const trimmed = selectCols.trim();
        if (!trimmed) {
            return { kind: "all" };
        }
        // `select_cols` in generated clients is a raw column-name string list;
        // split on commas.  The Rust WASM serde just passes through.
        return {
            kind: "columns",
            columns: trimmed.split(",").map((s) => s.trim()).filter(Boolean),
        };
    }
    return { kind: "all" };
}

/**
 * Lift a build-time `D1PhysicalPlan` + runtime `OperationRequest` into the
 * fully-specified `D1Request` envelope that the WASM bridge expects.
 *
 * This is the last TS-side construct point — SQL lowering and capability
 * checks live in the Rust build-time adapter; we only fill the runtime
 * parameters as WireValues.
 */
function buildD1Request(
    plan: D1PhysicalPlan,
    request: OperationRequest,
): D1Request {
    const params: WireValue[] = (plan.paramOrder ?? []).map((key) =>
        jsValueToWireValue(request.parameters?.[key]),
    );

    // Also append any non-param meta keys (currently only `select_cols` which
    // already went into `RETURNING` — kept as a forward-compat note).

    return {
        sql: plan.sql,
        params,
        mode: planModeToD1Mode(plan.mode),
        returning: buildReturningProjection(plan, request.parameters),
        contract_fingerprint: request.identity.contractFingerprint,
    };
}

// ---------------------------------------------------------------------------
// Diagnostics helper
// ---------------------------------------------------------------------------

function notWired(code: string, message: string): ResultEnvelope<never> {
    return {
        ok: false,
        diagnostics: [{ code, message, severity: "error" }],
    };
}

function canonicalRowsToExecutionRows(result: CanonicalResult): readonly ExecutionRow[] {
    return result.rows.map((row: Readonly<Record<string, unknown>>) => canonicalRowToCarrierRow(row) as ExecutionRow);
}

// ---------------------------------------------------------------------------
// Executor factory
// ---------------------------------------------------------------------------

/**
 * Create a Cloudflare Worker D1-backed `OperationExecutor`.
 *
 * @param d1          Duck-typed Cloudflare D1 database surface (`env.DB`).
 * @param plans       Build-time `D1PhysicalPlan` registry from generated
 *                    `_internal/d1-plans.ts`.
 * @param wireNamesByEntity  Wire-name map from generated `metadata.ts`.
 * @param valueCarriersByEntity  Value-carrier map from generated `metadata.ts`.
 *
 * `wireNamesByEntity` and `valueCarriersByEntity` are **kept for shape
 * compatibility** with the existing executor signature but are no longer
 * consumed here — canonical VOS Value → JS carrier mapping now happens in
 * the WASM bridge, which already knows the VOS type system.
 */
export function createD1OperationExecutor(

    d1: D1HostDb,

    plans: D1PlanRegistry = {},

    _wireNamesByEntity: Readonly<Record < string, Readonly < Record < string, string >>> > = {},

    _valueCarriersByEntity: Readonly<Record < string, Readonly < Record < string, string >>> > = {},

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

            if (plan.mode === "write") {

                return notWired(

                    "IRIS-D1-PLAN-INVALID",

                    `D1 plan for ${request.identity.operationId} is a plain write — use executeUnit()`,

                );

            }

            try {

                const d1Req = buildD1Request(plan, request);

                const canonical = await executeD1Request(d1Req, d1);

                const value = canonicalRowsToExecutionRows(canonical);

                return { ok: true, value };

            } catch (error) {

                const message = error instanceof Error ? error.message : String(error);

                return notWired("IRIS-D1-EXEC-FAILED", message);

            }

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

                    `D1 plan for ${request.identity.operationId} is not a plain write (mode=${plan.mode})`,

                );

            }

            try {

                const d1Req = buildD1Request(plan, request);

                await executeD1Request(d1Req, d1);

                return { ok: true, value: undefined };

            } catch (error) {

                const message = error instanceof Error ? error.message : String(error);

                return notWired("IRIS-D1-EXEC-FAILED", message);

            }

        },

        async close() {},

    };

}



/** @deprecated Use `createD1OperationExecutor`. */

export const createD1ReadOperationExecutor = createD1OperationExecutor;
