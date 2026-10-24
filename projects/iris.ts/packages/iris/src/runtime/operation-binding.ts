import type { MemorySessionBinding, SessionExecuteWire } from "../bindings.ts";
import type { OperationRequest, ResultEnvelope } from "../types/contract.ts";
import type { ExecutionRow } from "../types/execution-result.ts";
import type { OperationExecutor } from "../types/operation-executor.ts";
import { parseExecuteJson, parseRowsJson } from "./parse.ts";
import {
    IrisParameterEncodingError,
    stringifyOperationParameters,
    stringifyOperationRequest,
} from "./operation-parameters.ts";
import { entityFromOperationId, mapRowsToAuthorSurface } from "./wire-row-map.ts";

export type OperationExecutorSessionOptions = {
    wireNamesByEntity?: Readonly<Record < string, Readonly < Record < string, string >>> >;
    /** Generated `IRIS_VALUE_CARRIERS` table (VOS carrier label per author field). */
    valueCarriersByEntity?: Readonly<Record < string, Readonly < Record < string, string >>> >;
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
    wireNamesByEntity: Readonly<Record < string, Readonly < Record < string, string >>> >,
    valueCarriersByEntity: Readonly<Record < string, Readonly < Record < string, string >>> >,
): ResultEnvelope<readonly ExecutionRow[]> {
    if (!envelope.ok) {
        return envelope;
    }
    const entity = entityFromOperationId(operationId);
    const wireToTs = entity ? wireNamesByEntity[entity] ?? {} : {};
    const valueCarriers = entity ? valueCarriersByEntity[entity] ?? {} : {};
    return { ok: true, value: mapRowsToAuthorSurface(envelope.value, wireToTs, valueCarriers) };
}

function encodingFailure(error: unknown): Extract<ResultEnvelope<never>, { ok: false }> {
    const message = error instanceof Error ? error.message : String(error);
    return {
        ok: false,
        diagnostics: [
            {
                code: error instanceof IrisParameterEncodingError ? error.code : "IRIS-PARAM-ENCODING",
                message,
                severity: "error",
            },
        ],
    };
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
    const parametersJson =
        request.parameters === undefined ? null : stringifyOperationParameters(request.parameters);
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
    const valueCarriersByEntity = options.valueCarriersByEntity ?? {};
    const runOperation = session.executeOperation?.bind(session);
    const runQuery = session.query ?? session.executeVos.bind(session);
    const runExecute = session.execute?.bind(session);

    return {
        async execute(request: OperationRequest): Promise<ResultEnvelope<readonly ExecutionRow[]>> {
            if (runOperation) {
                let requestJson: string;
                try {
                    requestJson = stringifyOperationRequest(request);
                } catch (error) {
                    return encodingFailure(error);
                }
                const raw = runOperation(requestJson);
                return mapExecuteEnvelope(
                    wireToEnvelope(raw),
                    request.identity.operationId,
                    wireNamesByEntity,
                    valueCarriersByEntity,
                );
            }
            let raw: SessionExecuteWire;
            try {
                raw = declaredVosWire(session, request, runQuery, runExecute, "query");
            } catch (error) {
                return encodingFailure(error);
            }
            return mapExecuteEnvelope(
                wireToEnvelope(raw),
                request.identity.operationId,
                wireNamesByEntity,
                valueCarriersByEntity,
            );
        },
        async executeUnit(request: OperationRequest): Promise<ResultEnvelope<void>> {
            let raw: SessionExecuteWire;
            try {
                if (request.operation.kind === "declared-vos" && runExecute) {
                    raw = declaredVosWire(session, request, runQuery, runExecute, "execute");
                } else if (runOperation) {
                    raw = runOperation(stringifyOperationRequest(request));
                } else {
                    raw = declaredVosWire(session, request, runQuery, runExecute, "execute");
                }
            } catch (error) {
                const failure = encodingFailure(error);
                return { ok: false, diagnostics: failure.diagnostics };
            }
            const envelope = wireToEnvelope(raw);
            return envelope.ok ? { ok: true, value: undefined } : { ok: false, diagnostics: envelope.diagnostics };
        },
        async close() {
            session.close();
        },
    };
}
