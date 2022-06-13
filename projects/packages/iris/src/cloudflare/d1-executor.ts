import type { OperationRequest, ResultEnvelope } from "../types/contract.ts";
import type { ExecutionRow } from "../types/execution-result.ts";
import type { OperationExecutor } from "../types/operation-executor.ts";
import type { IrisD1Database } from "./types.ts";

function notWired(code: string, message: string): ResultEnvelope<never> {
    return {
        ok: false,
        diagnostics: [{ code, message, severity: "error" }],
    };
}

/**
 * Read-only D1 operation executor stub.
 *
 * Phase 3 wires build-time physical plan artifacts here. Until then callers receive
 * structured diagnostics instead of silent SQL passthrough.
 */
export function createD1ReadOperationExecutor(d1: IrisD1Database): OperationExecutor {
    void d1;
    return {
        async execute(_request: OperationRequest): Promise<ResultEnvelope<readonly ExecutionRow[]>> {
            return notWired(
                "IRIS-D1-READ-NOT-WIRED",
                "D1 read path requires a build-time physical plan artifact (not wired yet)",
            );
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
