import type { IrisBindings, MemorySessionBinding, OpenSessionNapiOptions } from "../bindings.ts";
import type { IrisBindingHost, IrisHost } from "../types/binding.ts";
import type { OperationRequest } from "../types/contract.ts";
import type { SchemaIntrospection } from "../types/schema-introspection.ts";
import type { IrisSession, OpenSessionOptions } from "../types/session.ts";
import { negotiateCapabilities } from "./negotiate-capabilities.ts";
import { parseExecuteJson, parseIntrospectionJson, parseRowsJson } from "./parse.ts";

export type { IrisBindings as SemanticCoreBinding, MemorySessionBinding, OpenSessionNapiOptions } from "../bindings.ts";

function parseWire(raw: string | { ok: boolean; rowsJson: string; error?: string | null }) {
    if (typeof raw === "string") {
        return parseExecuteJson(raw);
    }
    return {
        ok: raw.ok,
        rows: parseRowsJson(raw.rowsJson),
        error: raw.error ?? null,
    };
}

function wrapSession(binding: MemorySessionBinding): IrisSession {
    const runQuery = binding.query ?? binding.executeVos.bind(binding);
    const session: IrisSession = {
        execute(source: string) {
            return parseWire(runQuery(source));
        },
        executeOperation(request: OperationRequest) {
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

function resolveSessionBinding(host: IrisHost, core: IrisBindings, options?: OpenSessionOptions): MemorySessionBinding {
    if (host === "node" && core.openSession) {
        const profile =
            options?.profile ??
            (options?.sqlitePath
                ? "sqlite"
                : options?.postgresUrl
                  ? "postgres"
                  : options?.mysqlUrl
                    ? "mysql"
                    : options?.project
                      ? "project"
                      : "memory");
        return core.openSession({
            profile,
            sqlitePath: options?.sqlitePath,
            postgresUrl: options?.postgresUrl,
            mysqlUrl: options?.mysqlUrl,
            projectConfig: options?.project,
            datasource: options?.source,
        });
    }

    if (host === "node") {
        if (options?.postgresUrl && core.openPostgresSession) {
            return core.openPostgresSession(options.postgresUrl);
        }
        if (options?.mysqlUrl && core.openMysqlSession) {
            return core.openMysqlSession(options.mysqlUrl);
        }
        if (options?.sqlitePath && core.openSqliteSession) {
            return core.openSqliteSession(options.sqlitePath);
        }
        if (options?.project && core.openProjectSession) {
            return core.openProjectSession(options.project, options.source ?? "default");
        }
    }

    return core.openMemorySession();
}

function sessionProfile(options?: OpenSessionOptions): "memory" | "sqlite" | "project" {
    if (options?.profile === "sqlite" || options?.sqlitePath) {
        return "sqlite";
    }
    if (options?.profile === "project" || options?.project) {
        return "project";
    }
    if (options?.postgresUrl || options?.mysqlUrl) {
        return "project";
    }
    return "memory";
}

/** Build the symmetric Iris runtime facade from a loaded semantic core. */
export function buildRuntime(host: IrisHost, core: IrisBindings, profile: "memory" | "sqlite" | "project" = "memory"): IrisBindingHost {
    return {
        host,
        capabilities: negotiateCapabilities({ host, profile, bindingReady: true }),
        version: () => core.irisVersion(),
        checkSource: (source) => core.checkSource(source),
        introspectSchema: (source): SchemaIntrospection => parseIntrospectionJson(core.introspectSchema(source)),
        openSession: (options?: OpenSessionOptions) => wrapSession(resolveSessionBinding(host, core, options)),
    };
}

/** Build runtime with capabilities derived from session open options. */
export function buildRuntimeForSession(host: IrisHost, core: IrisBindings, options?: OpenSessionOptions): IrisBindingHost {
    return buildRuntime(host, core, sessionProfile(options));
}
