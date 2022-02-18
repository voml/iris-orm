import { createRequire } from "node:module";

import type { CheckSourceResult } from "../types/check-source.ts";
import { IrisFacadeError } from "../types/errors.ts";
import type { SemanticCoreBinding, MemorySessionBinding } from "../runtime/build-runtime.ts";

const require = createRequire(import.meta.url);

type LoadProjectResult = {
    root: string;
    config: string;
    schemaGlob: string;
    generateOut: string;
    generateTarget: string;
};

type GenerateResult = {
    ok: boolean;
    outputPath: string;
    schemaFingerprint: string;
    files: string[];
    error?: string | null;
};

type MigratePlanResult = {
    ok: boolean;
    planPath: string;
    error?: string | null;
};

type NativeMemorySession = {
    executeVos(source: string, parametersJson?: string | null): { ok: boolean; rowsJson: string; error?: string | null };
    executeOperation?(operationJson: string): { ok: boolean; rowsJson: string; error?: string | null };
    close(): void;
    managedPush?: (schema: string) => void;
};

type OpenSessionNapiOptions = {
    profile?: string;
    sqlitePath?: string;
    postgresUrl?: string;
    mysqlUrl?: string;
    projectConfig?: string;
    datasource?: string;
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

export type SemanticCore = SemanticCoreBinding & {
    loadProject(configPath: string): LoadProjectResult;
    readSchema(projectRoot: string, schemaGlob: string): string;
    generate(source: string, target: string, outDir: string): GenerateResult;
    migratePlanCmd(configPath: string, source: string, outDir?: string | null): MigratePlanResult;
};

let cached: SemanticCore | null = null;

function resolvePlatformPackage(platform: NodeJS.Platform, arch: string): string | null {
    if (platform === "win32" && arch === "x64") {
        return "@yydb/iris-win32-x64";
    }
    if (platform === "linux" && arch === "x64") {
        return "@yydb/iris-linux-x64";
    }
    return null;
}

function isPackageInstalled(packageName: string): boolean {
    try {
        require.resolve(packageName);
        return true;
    } catch {
        return false;
    }
}

function loadModule(specifier: string): SemanticCore {
    try {
        const module = require(specifier) as Record<string, unknown>;
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
            readSchema: (projectRoot, schemaGlob) => String((module.readSchema as (r: string, g: string) => string)(projectRoot, schemaGlob)),
            generate: (source, target, outDir) =>
                (module.generate as (s: string, t: string, o: string) => GenerateResult)(source, target, outDir),
            migratePlanCmd: (configPath, source, outDir) =>
                (module.migratePlanCmd as (c: string, s: string, o?: string | null) => MigratePlanResult)(
                    configPath,
                    source,
                    outDir ?? undefined,
                ),
        };
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        throw new IrisFacadeError("native-load-failed", `@yydb/iris/node: failed to load semantic core (${message})`);
    }
}

/** Whether the optional Node semantic core resolves for this host. */
export function isNodeSemanticCoreInstalled(platform: NodeJS.Platform = process.platform, arch: string = process.arch): boolean {
    const packageName = resolvePlatformPackage(platform, arch);
    return packageName != null && isPackageInstalled(packageName);
}

/** Whether the optional browser semantic core resolves. */
export function isBrowserSemanticCoreInstalled(): boolean {
    return isPackageInstalled("@yydb/iris-unknown-wasm32");
}

/** Load the Rust semantic core for the current Node host. */
export async function loadSemanticCore(): Promise<SemanticCore> {
    if (cached) {
        return cached;
    }

    const platform = process.platform;
    const arch = process.arch;
    const overridePath = process.env.NAPI_RS_NATIVE_LIBRARY_PATH;

    if (overridePath) {
        cached = loadModule(overridePath);
        return cached;
    }

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

    cached = loadModule(packageName);
    return cached;
}
