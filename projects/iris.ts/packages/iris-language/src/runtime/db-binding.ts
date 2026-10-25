import type {
    WebExecutionWireResult,
    WebIrisDbBinding,
    WebMemorySessionBinding,
    WebSessionExecuteWire,
    WebVosParameters,
} from "@yydb/iris-unknown-wasm32";

import { parseExecuteJson, parseRowsJson } from "./parse.ts";
import { stringifyOperationParameters } from "./operation-parameters.ts";

/** Session surface consumed by generated browser `db` bindings. */
export type DbBindingSession = WebMemorySessionBinding;

function parametersJson(parameters?: WebVosParameters): string | null {
    return parameters === undefined ? null : stringifyOperationParameters(parameters);
}

function wireToRows(raw: WebSessionExecuteWire): WebExecutionWireResult {
    if (typeof raw === "string") {
        const parsed = parseExecuteJson(raw);
        if (!parsed.ok) {
            throw new Error(parsed.error ?? "iris execution failed");
        }
        return { kind: "rows", rows: parsed.rows };
    }
    if (!raw.ok) {
        throw new Error(raw.error ?? "iris execution failed");
    }
    const rows = parseRowsJson(raw.rowsJson);
    return { kind: "rows", rows };
}

function runQuery(session: DbBindingSession, source: string, parameters?: WebVosParameters): WebExecutionWireResult {
    const json = parametersJson(parameters);
    if (session.query) {
        return wireToRows(session.query(source, json));
    }
    return wireToRows(session.executeVos(source, json));
}

function runExecute(session: DbBindingSession, source: string, parameters?: WebVosParameters): void {
    const json = parametersJson(parameters);
    if (session.execute) {
        wireToRows(session.execute(source, json));
        return;
    }
    runQuery(session, source, parameters);
}

/** Map binding wire shape to the VOS value surfaced by `db.$query`. */
function mapWireToQueryValue(wire: WebExecutionWireResult): unknown {
    switch (wire.kind) {
        case "rows":
            return wire.rows;
        case "value":
            return wire.value;
        case "affected":
            return wire.affected;
        case "unit":
            return undefined;
    }
}

/** Build the generated-client binding from an open session. */
export function createIrisDbBindingFromSession(session: DbBindingSession): WebIrisDbBinding {
    return {
        async query(source: string, parameters?: WebVosParameters): Promise<unknown> {
            return mapWireToQueryValue(runQuery(session, source, parameters));
        },
        async execute(source: string, parameters?: WebVosParameters): Promise<void> {
            runExecute(session, source, parameters);
        },
        async close() {
            session.close();
        },
    };
}
