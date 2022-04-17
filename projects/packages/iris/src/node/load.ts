import { createRequire } from "node:module";

import type { CheckSourceResult } from "../types/check-source.ts";
import { IrisFacadeError } from "../types/errors.ts";
import type { IrisNodeBindings, MemorySessionBinding, OpenSessionNapiOptions } from "../bindings.ts";
import type {
    GenerateResult,
    LoadProjectResult,
    MigratePlanResult,
} from "../bindings.ts";
import { resolvePlatformPackage } from "./platform-packages.ts";

const require = createRequire(import.meta.url);

type NativeMemorySession = {
    executeVos(source: string, parametersJson?: string | null): { ok: boolean; rowsJson: string; error?: string | null };
    executeOperation?(operationJson: string): { ok: boolean; rowsJson: string; error?: string | null };
    close(): void;
    managedPush?: (schema: string) => void;
};

function wrapNativeSession(session: NativeMemorySession): MemorySessionBinding {
    const binding: MemorySessionBinding = {
        executeVos: (source: string, parametersJson?: string | null) => session.executeVos(source, parametersJson ?? null),
        close: () => session.close(),
    };
    if (session.executeOperation) {
        binding.executeOperation = (operationJson: string) => session.executeOperation!(operationJson);
    }
    if (session.managedPush) {
        binding.managedPush = (schema: string) => session.managedPush!(schema);
    }
    return binding;
}

function loadModule(specifier: string): IrisNodeBindings {
    try {
        const loaded = require(specifier) as Record<string, unknown> & { default?: Record<string, unknown> };
        const module = (loaded.default ?? loaded) as Record<string, unknown>;
        return {
            irisVersion: () => String((module.irisVersion as () => string)()),
            checkSource: (source) => (module.checkSource as (s: string) => CheckSourceResult)(source),
            introspectSchema: (source) => String((module.introspectSchema as (s: string) => string)(source)),
            openMemorySession: () => {
                const session = (module.openMemorySession as () => NativeMemorySession)();
                return wrapNativeSession(session);
            },
            openSession: (options?: OpenSessionNapiOptions) => {
                const session = (module.openSession as (o?: OpenSessionNapiOptions) => NativeMemorySession)(options);
                return wrapNativeSession(session);
            },
            openSqliteSession: (path) => {
                const session = (module.openSqliteSession as (p: string) => NativeMemorySession)(path);
                return wrapNativeSession(session);
            },
            openPostgresSession: (url) => {
                const session = (module.openPostgresSession as (u: string) => NativeMemorySession)(url);
                return wrapNativeSession(session);
            },
            openMysqlSession: (url) => {
                const session = (module.openMysqlSession as (u: string) => NativeMemorySession)(url);
                return wrapNativeSession(session);
            },
            openProjectSession: (configPath, source) => {
                const session = (module.openProjectSession as (c: string, s: string) => NativeMemorySession)(configPath, source);
                return wrapNativeSession(session);
            },
            loadProject: (configPath) => (module.loadProject as (p: string) => LoadProjectResult)(configPath),
            materializeRuntimeProject: (projectDir, documentJson) =>
                String(
                    (module.materializeRuntimeProject as (d: string, j: string) => string)(projectDir, documentJson),
                ),
            readSchema: (projectRoot, schemaGlob) => String((module.readSchema as (r: string, g: string) => string)(projectRoot, schemaGlob)),
            generate: (source, target, outDir) =>
                (module.generate as (s: string, t: string, o: string) => GenerateResult)(source, target, outDir),
            migratePlanCmd: (configPath, source, outDir) =>
                (module.migratePlanCmd as (c: string, s: string, o?: string | null) => MigratePlanResult)(
                    configPath,
                    source,
                    outDir ?? undefined,
                ),
            migrateRunCmd:
                typeof module.migrateRunCmd === "function"
                    ? (configPath, source, planOut, planOnly) =>
                          (
                              module.migrateRunCmd as (
                                  c: string,
                                  s: string,
                                  p?: string | null,
                                  o?: boolean,
                              ) => {
                                  ok: boolean;
                                  planPath: string;
                                  planOnly: boolean;
                                  createdTables: string[];
                                  error?: string | null;
                              }
                          )(configPath, source, planOut ?? undefined, planOnly ?? false)
                    : undefined,
        };
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        throw new IrisFacadeError("native-load-failed", `@yydb/iris/node: failed to load semantic core (${message})`);
    }
}

function isPackageInstalled(packageName: string): boolean {
    try {
        require.resolve(packageName);
        return true;
    } catch {
        return false;
    }
}

/** Whether the optional Node semantic core resolves for this host. */
export function isNodeSemanticCoreInstalled(platform: NodeJS.Platform = process.platform, arch: string = process.arch): boolean {
    const packageName = resolvePlatformPackage(platform, arch);
    return packageName != null && isPackageInstalled(packageName);
}

/** Load the Rust semantic core from the optional `@yydb/iris-<platform>` package. */
export function loadIrisNode(): IrisNodeBindings {
    const overridePath = process.env.NAPI_RS_NATIVE_LIBRARY_PATH;
    if (overridePath) {
        return loadModule(overridePath);
    }

    const platform = process.platform;
    const arch = process.arch;
    const packageName = resolvePlatformPackage(platform, arch);
    if (!packageName) {
        throw new IrisFacadeError("native-unsupported-platform", `@yydb/iris/node: no semantic core published for ${platform}-${arch}`);
    }
    if (!isPackageInstalled(packageName)) {
        throw new IrisFacadeError(
            "native-package-missing",
            `@yydb/iris/node: semantic core not installed for ${platform}-${arch} (reinstall @yydb/iris with optional dependencies)`,
        );
    }

    return loadModule(packageName);
}

let cached: IrisNodeBindings | undefined;

/** Cached Node-API binding loader (panduck `loadPanduckNative` shape). */
export function loadIrisNative(): IrisNodeBindings {
    if (!cached) {
        cached = loadIrisNode();
    }
    return cached;
}
