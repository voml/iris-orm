import type { MemorySessionBinding, SessionExecuteWire } from "../bindings.ts";
import type { OperationRequest, ResultEnvelope } from "../types/contract.ts";
import type { ExecutionRow } from "../types/execution-result.ts";
import type { OperationExecutor } from "../types/operation-executor.ts";
import { parseExecuteJson, parseRowsJson } from "./parse.ts";

function wireToEnvelope(raw: SessionExecuteWire): ResultEnvelope<readonly ExecutionRow[]> {
    if (typeof raw === "string") {
        const parsed = parseExecuteJson(raw);
        if (!parsed.ok) {
            return {
                ok: false,
                diagnostics: [
                    {
                        code: "IRIS-EXEC-FAILED",
                        message: parsed.error ?? "iris execution failed",
                        severity: "error",
                    },
                ],
            };
        }
        return { ok: true, value: parsed.rows };
    }
    if (!raw.ok) {
        return {
            ok: false,
            diagnostics: [
                {
                    code: "IRIS-EXEC-FAILED",
                    message: raw.error ?? "iris execution failed",
                    severity: "error",
                },
            ],
        };
    }
    return { ok: true, value: parseRowsJson(raw.rowsJson) };
}

/** Build the async operation executor from an open in-process session. */
export function createOperationExecutorFromSession(session: MemorySessionBinding): OperationExecutor {
    const runOperation = session.executeOperation?.bind(session);
    const runQuery = session.query ?? session.executeVos.bind(session);

    return {
        async execute(request: OperationRequest): Promise<ResultEnvelope<readonly ExecutionRow[]>> {
            const payload = JSON.stringify(request.operation);
            const raw = runOperation ? runOperation(payload) : runQuery(payload);
            return wireToEnvelope(raw);
        },
        async close() {
            session.close();
        },
    };
}
