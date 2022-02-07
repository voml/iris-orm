import type { CheckSourceResult } from "../types/check-source.ts";
import type { IrisBindingHost, IrisHost } from "../types/binding.ts";
import type { IrisOperation } from "../types/operation.ts";
import type { SchemaIntrospection } from "../types/schema-introspection.ts";
import type { IrisSession, OpenSessionOptions } from "../types/session.ts";
import { parseExecuteJson, parseIntrospectionJson, parseRowsJson } from "./parse.ts";

export type MemorySessionBinding = {
    executeVos(
        source: string,
        parametersJson?: string | null,
    ): string | { ok: boolean; rowsJson: string; error?: string | null };
    executeOperation?(operationJson: string): string | { ok: boolean; rowsJson: string; error?: string | null };
    close(): void;
    managedPush?: (schema: string) => void;
};

export type SemanticCoreBinding = {
    irisVersion(): string;
    checkSource(source: string): CheckSourceResult;
    introspectSchema(source: string): string;
    openMemorySession(): MemorySessionBinding;
    openSession?(options?: OpenSessionNapiOptions): MemorySessionBinding;
    openSqliteSession?(path: string): MemorySessionBinding;
    openPostgresSession?(url: string): MemorySessionBinding;
    openMysqlSession?(url: string): MemorySessionBinding;
    openProjectSession?(configPath: string, source: string): MemorySessionBinding;
};

/** N-API `openSession` wire shape (camelCase from napi-rs). */
export type OpenSessionNapiOptions = {
    profile?: string;
    sqlitePath?: string;
    postgresUrl?: string;
    mysqlUrl?: string;
    projectConfig?: string;
    datasource?: string;
};

function wrapSession(binding: MemorySessionBinding): IrisSession {
    const session: IrisSession = {
        execute(source: string) {
            const raw = binding.executeVos(source);
            if (typeof raw === "string") {
                return parseExecuteJson(raw);
            }
            return {
                ok: raw.ok,
                rows: parseRowsJson(raw.rowsJson),
                error: raw.error ?? null,
            };
        },
        executeOperation(operation: IrisOperation) {
            const json = JSON.stringify(operation);
            const raw = binding.executeOperation
                ? binding.executeOperation(json)
                : binding.executeVos(json);
            if (typeof raw === "string") {
                return parseExecuteJson(raw);
            }
            return {
                ok: raw.ok,
                rows: parseRowsJson(raw.rowsJson),
                error: raw.error ?? null,
            };
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

function resolveSessionBinding(host: IrisHost, core: SemanticCoreBinding, options?: OpenSessionOptions): MemorySessionBinding {
    if (host === "node" && core.openSession) {
        const profile = options?.profile ?? (options?.sqlitePath ? "sqlite" : options?.postgresUrl ? "postgres" : options?.mysqlUrl ? "mysql" : options?.project ? "project" : "memory");
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

/** Build the symmetric Iris runtime facade from a loaded semantic core. */
export function buildRuntime(host: IrisHost, core: SemanticCoreBinding): IrisBindingHost {
    return {
        host,
        capabilities: {
            host,
            bindingReady: true,
        },
        version: () => core.irisVersion(),
        checkSource: (source) => core.checkSource(source),
        introspectSchema: (source): SchemaIntrospection => parseIntrospectionJson(core.introspectSchema(source)),
        openSession: (options?: OpenSessionOptions) => wrapSession(resolveSessionBinding(host, core, options)),
    };
}
