import type { CreateIrisDbBindingOptions, IrisDbBinding } from "../types/executor.ts";
import { mapWireToQueryValue } from "../types/executor.ts";
import type { ExecutionWireResult } from "../types/execution-result.ts";
import { parseExecuteJson, parseRowsJson } from "../runtime/parse.ts";
import type { IrisNodeBindings } from "../bindings.ts";
import { loadIrisNative } from "./load.ts";
import { loadProject, readProjectSchema } from "./project.ts";

type NativeSession = {
    executeVos(source: string, parametersJson?: string | null): { ok: boolean; rowsJson: string; error?: string | null };
    close(): void;
    managedPush?: (schema: string) => void;
};

async function openNativeSession(core: IrisNodeBindings, options?: CreateIrisDbBindingOptions): Promise<NativeSession> {
    const configPath = options?.config ?? options?.project;
    const profile =
        options?.profile ??
        (options?.sqlitePath ? "sqlite" : configPath ? "project" : "memory");
    let session: NativeSession;
    let schemaData = options?.schema;
    if (profile === "sqlite" && core.openSqliteSession) {
        session = core.openSqliteSession(options?.sqlitePath ?? ":memory:") as NativeSession;
    } else if (profile === "project" && core.openProjectSession && configPath) {
        const project = await loadProject(configPath);
        session = core.openProjectSession(project.runtimeConfig, options?.source ?? "default") as NativeSession;
        if (!schemaData) {
            schemaData = await readProjectSchema(project);
        }
    } else {
        session = core.openMemorySession() as NativeSession;
    }
    if (schemaData && session.managedPush) {
        session.managedPush(schemaData);
    }
    return session;
}

function runVos(session: NativeSession, source: string, parameters?: Readonly<Record<string, unknown>>): ExecutionWireResult {
    // Explicit parameters object (even `{}`) goes through Rust binder so unbound `$name` fails.
    const parametersJson = parameters === undefined ? null : JSON.stringify(parameters);
    const raw = session.executeVos(source, parametersJson);
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

/** Create internal binding support for generated `db` (not an application entry). */
export async function createIrisDbBinding(options: CreateIrisDbBindingOptions = {}): Promise<IrisDbBinding> {
    const core = loadIrisNative();
    const session = await openNativeSession(core, options);

    return {
        async query(source: string, parameters?: Readonly<Record<string, unknown>>): Promise<unknown> {
            return mapWireToQueryValue(runVos(session, source, parameters));
        },
        async execute(source: string, parameters?: Readonly<Record<string, unknown>>): Promise<void> {
            runVos(session, source, parameters);
            // Scaffold: Rust will classify DDL vs DML and return explicit unit.
        },
        async close() {
            session.close();
        },
    };
}

/** @deprecated Use `createIrisDbBinding`. */
export const createIrisExecutor = createIrisDbBinding;

/** @deprecated Binding bring-up host; use generated `db`. */
export async function createIrisBindingHost(options: CreateIrisDbBindingOptions = {}) {
    const { buildRuntime } = await import("../runtime/build-runtime.ts");
    const core = loadIrisNative();
    return buildRuntime("node", core);
}
