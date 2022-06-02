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

function declaredVosWire(
    session: MemorySessionBinding,
    request: OperationRequest,
    runQuery: (source: string, parametersJson?: string | null) => SessionExecuteWire,
    runExecute: ((source: string, parametersJson?: string | null) => SessionExecuteWire) | undefined,
    mode: "query" | "execute",
): SessionExecuteWire {
    if (request.operation.kind !== "declared-vos") {
        return { ok: false, rowsJson: "[]", error: "session fallback requires declared-vos operations" };
    }
    const parametersJson = request.parameters === undefined ? null : JSON.stringify(request.parameters);
    if (mode === "execute" && runExecute) {
        return runExecute(request.operation.source, parametersJson);
    }
    return runQuery(request.operation.source, parametersJson);
}

/** Build the async operation executor from an open in-process session. */
export function createOperationExecutorFromSession(session: MemorySessionBinding): OperationExecutor {
    const runOperation = session.executeOperation?.bind(session);
    const runQuery = session.query ?? session.executeVos.bind(session);
    const runExecute = session.execute?.bind(session);

    return {
        async execute(request: OperationRequest): Promise<ResultEnvelope<readonly ExecutionRow[]>> {
            if (runOperation) {
                const raw = runOperation(JSON.stringify(request));
                return wireToEnvelope(raw);
            }
            const raw = declaredVosWire(session, request, runQuery, runExecute, "query");
            return wireToEnvelope(raw);
        },
        async executeUnit(request: OperationRequest): Promise<ResultEnvelope<void>> {
            if (runOperation) {
                const raw = runOperation(JSON.stringify(request));
                const envelope = wireToEnvelope(raw);
                return envelope.ok ? { ok: true, value: undefined } : { ok: false, diagnostics: envelope.diagnostics };
            }
            const raw = declaredVosWire(session, request, runQuery, runExecute, "execute");
            const envelope = wireToEnvelope(raw);
            return envelope.ok ? { ok: true, value: undefined } : { ok: false, diagnostics: envelope.diagnostics };
        },
        async close() {
            session.close();
        },
    };
}
