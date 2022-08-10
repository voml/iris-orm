import type { MemorySessionBinding, SessionExecuteWire } from "../bindings.ts";
import type { OperationRequest, ResultEnvelope } from "../types/contract.ts";
import type { ExecutionRow } from "../types/execution-result.ts";
import type { OperationExecutor } from "../types/operation-executor.ts";
import { parseExecuteJson, parseRowsJson } from "./parse.ts";
import { entityFromOperationId, mapRowsToAuthorSurface } from "./wire-row-map.ts";

export type OperationExecutorSessionOptions = {
    wireNamesByEntity?: Readonly<Record<string, Readonly<Record<string, string>>>>;
};

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

function mapExecuteEnvelope(
    envelope: ResultEnvelope<readonly ExecutionRow[]>,
    operationId: string,
    wireNamesByEntity: Readonly<Record<string, Readonly<Record<string, string>>>>,
): ResultEnvelope<readonly ExecutionRow[]> {
    if (!envelope.ok) {
        return envelope;
    }
    const entity = entityFromOperationId(operationId);
    const wireToTs = entity ? wireNamesByEntity[entity] ?? {} : {};
    return { ok: true, value: mapRowsToAuthorSurface(envelope.value, wireToTs) };
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
export function createOperationExecutorFromSession(
    session: MemorySessionBinding,
    options: OperationExecutorSessionOptions = {},
): OperationExecutor {
    const wireNamesByEntity = options.wireNamesByEntity ?? {};
    const runOperation = session.executeOperation?.bind(session);
    const runQuery = session.query ?? session.executeVos.bind(session);
    const runExecute = session.execute?.bind(session);

    return {
        async execute(request: OperationRequest): Promise<ResultEnvelope<readonly ExecutionRow[]>> {
            if (runOperation) {
                const raw = runOperation(JSON.stringify(request));
                return mapExecuteEnvelope(wireToEnvelope(raw), request.identity.operationId, wireNamesByEntity);
            }
            const raw = declaredVosWire(session, request, runQuery, runExecute, "query");
            return mapExecuteEnvelope(wireToEnvelope(raw), request.identity.operationId, wireNamesByEntity);
        },
        async executeUnit(request: OperationRequest): Promise<ResultEnvelope<void>> {
            if (request.operation.kind === "declared-vos" && runExecute) {
                const raw = declaredVosWire(session, request, runQuery, runExecute, "execute");
                const envelope = wireToEnvelope(raw);
                return envelope.ok ? { ok: true, value: undefined } : { ok: false, diagnostics: envelope.diagnostics };
            }
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
