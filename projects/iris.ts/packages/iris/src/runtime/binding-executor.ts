import type { OperationRequest, ResultEnvelope } from "../types/contract.ts";
import type { ExecutionRow } from "../types/execution-result.ts";
import type { IrisDbBinding } from "../types/executor.ts";
import type { OperationExecutor } from "../types/operation-executor.ts";

/** Adapt legacy `IrisDbBinding` into `OperationExecutor` (declared-vos only). */
export function createOperationExecutorFromBinding(binding: IrisDbBinding): OperationExecutor {
    return {
        async execute(request: OperationRequest): Promise<ResultEnvelope<readonly ExecutionRow[]>> {
            if (request.operation.kind !== "declared-vos") {
                return {
                    ok: false,
                    diagnostics: [
                        {
                            code: "IRIS-UNSUPPORTED-OPERATION",
                            message: "legacy binding adapter only supports declared-vos operations",
                            severity: "error",
                        },
                    ],
                };
            }
            try {
                const value = await binding.query(request.operation.source, request.parameters);
                const rows = Array.isArray(value) ? (value as ExecutionRow[]) : [];
                return { ok: true, value: rows };
            } catch (error) {
                return {
                    ok: false,
                    diagnostics: [
                        {
                            code: "IRIS-EXEC-FAILED",
                            message: error instanceof Error ? error.message : String(error),
                            severity: "error",
                        },
                    ],
                };
            }
        },
        async executeUnit(request: OperationRequest): Promise<ResultEnvelope<void>> {
            if (request.operation.kind !== "declared-vos") {
                return {
                    ok: false,
                    diagnostics: [
                        {
                            code: "IRIS-UNSUPPORTED-OPERATION",
                            message: "legacy binding adapter only supports declared-vos operations",
                            severity: "error",
                        },
                    ],
                };
            }
            try {
                await binding.execute(request.operation.source, request.parameters);
                return { ok: true, value: undefined };
            } catch (error) {
                return {
                    ok: false,
                    diagnostics: [
                        {
                            code: "IRIS-EXEC-FAILED",
                            message: error instanceof Error ? error.message : String(error),
                            severity: "error",
                        },
                    ],
                };
            }
        },
        async close() {
            await binding.close();
        },
    };
}
