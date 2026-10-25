import type {
    WebIrisBindingHost,
    WebIrisBindingProfile,
    WebIrisBindings,
    WebIrisHost,
    WebIrisSession,
    WebMemorySessionBinding,
    WebOpenSessionOptions,
    WebSchemaIntrospection,
    WebSessionExecuteWire,
} from "@yydb/iris-unknown-wasm32";

import { negotiateCapabilities } from "./negotiate-capabilities.ts";
import { parseExecuteJson, parseIntrospectionJson, parseRowsJson } from "./parse.ts";

function parseWire(raw: WebSessionExecuteWire) {
    if (typeof raw === "string") {
        return parseExecuteJson(raw);
    }
    return {
        ok: raw.ok,
        rows: parseRowsJson(raw.rowsJson),
        error: raw.error ?? null,
    };
}

function wrapSession(binding: WebMemorySessionBinding): WebIrisSession {
    const runQuery = binding.query ?? binding.executeVos.bind(binding);
    const session: WebIrisSession = {
        execute(source: string) {
            return parseWire(runQuery(source));
        },
        executeOperation(request) {
            const json = JSON.stringify(request);
            const raw = binding.executeOperation ? binding.executeOperation(json) : runQuery(json);
            return parseWire(raw);
        },
        close() {
            binding.close();
        },
    };
    if (binding.managedPush) {
        session.managedPush = (schema) => binding.managedPush!(schema);
    }
    return session;
}

/** Build the browser / Worker runtime facade from a loaded WASM semantic core. */
export function buildRuntime(
    host: WebIrisHost,
    core: WebIrisBindings,
    profile: WebIrisBindingProfile = "memory",
): WebIrisBindingHost {
    return {
        host,
        capabilities: negotiateCapabilities({ host, profile, bindingReady: true }),
        version: () => core.irisVersion(),
        checkSource: (source) => core.checkSource(source),
        introspectSchema: (source): WebSchemaIntrospection => parseIntrospectionJson(core.introspectSchema(source)),
        openSession: (_options?: WebOpenSessionOptions) => wrapSession(core.openMemorySession()),
    };
}
