import type { CheckSourceResult } from "./types/check-source.ts";

/** Wire session surface shared by Node N-API and browser WASM hosts. */
export type MemorySessionBinding = {
    executeVos(
        source: string,
        parametersJson?: string | null,
    ): string | { ok: boolean; rowsJson: string; error?: string | null };
    executeOperation?(operationJson: string): string | { ok: boolean; rowsJson: string; error?: string | null };
    close(): void;
    managedPush?: (schema: string) => void;
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

/** Shared semantic core consumed by `buildRuntime` on every host. */
export type IrisBindings = {
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

export type LoadProjectResult = {
    root: string;
    config: string;
    schemaGlob: string;
    generateOut: string;
    generateTarget: string;
};

export type GenerateResult = {
    ok: boolean;
    outputPath: string;
    schemaFingerprint: string;
    files: string[];
    error?: string | null;
};

export type MigratePlanResult = {
    ok: boolean;
    planPath: string;
    error?: string | null;
};

/** Node-only semantic core (tooling / project / migrate). */
export type IrisNodeBindings = IrisBindings & {
    loadProject(configPath: string): LoadProjectResult;
    readSchema(projectRoot: string, schemaGlob: string): string;
    generate(source: string, target: string, outDir: string): GenerateResult;
    migratePlanCmd(configPath: string, source: string, outDir?: string | null): MigratePlanResult;
    migrateRunCmd?(
        configPath: string,
        source: string,
        planOut?: string | null,
        planOnly?: boolean,
    ): {
        ok: boolean;
        planPath: string;
        planOnly: boolean;
        createdTables: string[];
        error?: string | null;
    };
};

/** Explicit WASM bytes or module handle for hosts with custom asset pipelines. */
export type WasmSource = URL | Request | Response | ArrayBuffer | Uint8Array | WebAssembly.Module;

export type IrisWasmOptions = {
    /** When omitted, the default optional semantic core asset is used. */
    module?: WasmSource;
};
