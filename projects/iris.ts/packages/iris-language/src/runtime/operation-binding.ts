import type {
    WebExecutionRow,
    WebIrisDiagnostic,
    WebMemorySessionBinding,
    WebOperationExecutor,
    WebOperationRequest,
    WebResultEnvelope,
    WebSessionExecuteWire,
} from "@yydb/iris-unknown-wasm32";

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

function wireToEnvelope(raw: WebSessionExecuteWire): WebResultEnvelope<readonly WebExecutionRow[]> {
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
    envelope: WebResultEnvelope<readonly WebExecutionRow[]>,
    operationId: string,
    wireNamesByEntity: Readonly<Record < string, Readonly < Record < string, string >>> >,
    valueCarriersByEntity: Readonly<Record < string, Readonly < Record < string, string >>> >,
): WebResultEnvelope<readonly WebExecutionRow[]> {
    if (!envelope.ok) {
        return envelope;
    }
    const entity = entityFromOperationId(operationId);
    const wireToTs = entity ? wireNamesByEntity[entity] ?? {} : {};
    const valueCarriers = entity ? valueCarriersByEntity[entity] ?? {} : {};
    return { ok: true, value: mapRowsToAuthorSurface(envelope.value, wireToTs, valueCarriers) };
}

function encodingFailure(error: unknown): Extract<WebResultEnvelope<never>, { ok: false }> {
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
    request: WebOperationRequest,
    runQuery: (source: string, parametersJson?: string | null) => WebSessionExecuteWire,
    runExecute: ((source: string, parametersJson?: string | null) => WebSessionExecuteWire) | undefined,
    mode: "query" | "execute",
): WebSessionExecuteWire {
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
    session: WebMemorySessionBinding,
    options: OperationExecutorSessionOptions = {},
): WebOperationExecutor {
    const wireNamesByEntity = options.wireNamesByEntity ?? {};
    const valueCarriersByEntity = options.valueCarriersByEntity ?? {};
    const runOperation = session.executeOperation?.bind(session);
    const runQuery = session.query ?? session.executeVos.bind(session);
    const runExecute = session.execute?.bind(session);

    return {
        async execute(request: WebOperationRequest): Promise<WebResultEnvelope<readonly WebExecutionRow[]>> {
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
            let raw: WebSessionExecuteWire;
            try {
                raw = declaredVosWire(request, runQuery, runExecute, "query");
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
        async executeUnit(request: WebOperationRequest): Promise<WebResultEnvelope<void>> {
            let raw: WebSessionExecuteWire;
            try {
                if (request.operation.kind === "declared-vos" && runExecute) {
                    raw = declaredVosWire(request, runQuery, runExecute, "execute");
                } else if (runOperation) {
                    raw = runOperation(stringifyOperationRequest(request));
                } else {
                    raw = declaredVosWire(request, runQuery, runExecute, "execute");
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
