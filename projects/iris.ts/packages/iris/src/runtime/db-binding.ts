import type { MemorySessionBinding, SessionExecuteWire } from "../bindings.ts";
import type { ExecutionWireResult } from "../types/execution-result.ts";
import type { IrisDbBinding, VosParameters } from "../types/executor.ts";
import { mapWireToQueryValue } from "../types/executor.ts";
import { parseExecuteJson, parseRowsJson } from "./parse.ts";

/** Session surface consumed by generated `db` bindings (Node N-API + browser WASM). */
export type DbBindingSession = MemorySessionBinding;

function parametersJson(parameters?: VosParameters): string | null {
    return parameters === undefined ? null : JSON.stringify(parameters);
}

function wireToRows(raw: SessionExecuteWire): ExecutionWireResult {
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

function runQuery(session: DbBindingSession, source: string, parameters?: VosParameters): ExecutionWireResult {
    const json = parametersJson(parameters);
    if (session.query) {
        return wireToRows(session.query(source, json));
    }
    return wireToRows(session.executeVos(source, json));
}

function runExecute(session: DbBindingSession, source: string, parameters?: VosParameters): void {
    const json = parametersJson(parameters);
    if (session.execute) {
        wireToRows(session.execute(source, json));
        return;
    }
    runQuery(session, source, parameters);
}

/** Build the symmetric generated-client binding from an open session. */
export function createIrisDbBindingFromSession(session: DbBindingSession): IrisDbBinding {
    return {
        async query(source: string, parameters?: VosParameters): Promise<unknown> {
            return mapWireToQueryValue(runQuery(session, source, parameters));
        },
        async execute(source: string, parameters?: VosParameters): Promise<void> {
            runExecute(session, source, parameters);
        },
        async close() {
            session.close();
        },
    };
}
