import type { CheckSourceResult } from "@yydb/iris/types";
import { IrisFacadeError } from "@yydb/iris/types";
import type { IrisBindings, MemorySessionBinding } from "@yydb/iris/runtime";

const WEB_CORE_PACKAGE = "@yydb/iris-unknown-wasm32";

/** Explicit WASM bytes or module handle for hosts with custom asset pipelines. */
export type WasmSource = URL | Request | Response | ArrayBuffer | Uint8Array | WebAssembly.Module;

export type IrisWasmOptions = {
    /** When omitted, the bundled `@yydb/iris-unknown-wasm32` asset is used. */
    module?: WasmSource;
};

/** @deprecated Use `IrisWasmOptions`. */
export type InitIrisOptions = IrisWasmOptions;

type WasmPackage = {
    initWasm(options?: { module?: WasmSource }): Promise<void>;
    irisVersion(): string;
    checkSource(source: string): CheckSourceResult;
    introspectSchema(source: string): string;
    openMemorySession(): WasmMemorySession;
    resetWasmBindingForTests(): void;
};

type WasmMemorySession = {
    query?(source: string, parametersJson?: string | null): string;
    execute?(source: string, parametersJson?: string | null): string;
    executeVos(source: string, parametersJson?: string | null): string;
    close(): void;
};

let cached: Promise<IrisBindings> | undefined;
let active: IrisBindings | null = null;

async function importWasmPackage(): Promise<WasmPackage> {
    return (await import(WEB_CORE_PACKAGE)) as WasmPackage;
}

function wrapWasmSession(session: WasmMemorySession): MemorySessionBinding {
    const executeVos = (source: string, parametersJson?: string | null) =>
        session.executeVos(source, parametersJson ?? null);
    return {
        query: session.query
            ? (source: string, parametersJson?: string | null) => session.query!(source, parametersJson ?? null)
            : executeVos,
        execute: session.execute
            ? (source: string, parametersJson?: string | null) => session.execute!(source, parametersJson ?? null)
            : executeVos,
        executeVos,
        close: () => session.close(),
    };
}

function toBindings(pkg: WasmPackage): IrisBindings {
    return {
        irisVersion: () => pkg.irisVersion(),
        checkSource: (source) => pkg.checkSource(source),
        introspectSchema: (source) => pkg.introspectSchema(source),
        openMemorySession: () => wrapWasmSession(pkg.openMemorySession()),
    };
}

/** Load and initialize the browser WASM semantic core (browser-safe dynamic import). */
export async function loadIrisWasm(options: IrisWasmOptions = {}): Promise<IrisBindings> {
    let pkg: WasmPackage;
    try {
        pkg = await importWasmPackage();
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        throw new IrisFacadeError(
            "wasm-package-missing",
            `@yydb/iris-language: browser semantic core not installed (install @yydb/iris-unknown-wasm32): ${message}`,
        );
    }

    try {
        await pkg.initWasm({ module: options.module });
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        throw new IrisFacadeError(
            "wasm-package-missing",
            `@yydb/iris-language: browser semantic core not built (run the wasm build): ${message}`,
        );
    }
    return toBindings(pkg);
}

/** Cached WASM binding loader (panduck `loadPanduckWeb` shape). */
export function loadIrisWeb(options: IrisWasmOptions = {}): Promise<IrisBindings> {
    if (cached) {
        return cached;
    }
    const loader = loadIrisWasm(options).then((bindings) => {
        active = bindings;
        return bindings;
    });
    cached = loader;
    return loader;
}

/** @deprecated Prefer `loadIrisWeb`. One-time WASM init before `createIris` on browser hosts. */
export async function initIris(options: IrisWasmOptions = {}): Promise<void> {
    await loadIrisWeb(options);
}

/** Active semantic core after `loadIrisWeb()` / `initIris()`. */
export function getWasmSemanticCore(): IrisBindings {
    if (active == null) {
        throw new IrisFacadeError(
            "wasm-not-initialized",
            "@yydb/iris-language: call initIris() or loadIrisWeb() before createIris()",
        );
    }
    return active;
}

/** @internal Reset init gate (binding tests only). */
export function resetInitStateForTests(): void {
    cached = undefined;
    active = null;
}

/**
 * Whether the browser semantic core package resolves (Node doctor / tooling only).
 *
 * Uses `import.meta.resolve` so this module stays browser-safe (no `node:module`).
 */
export function isBrowserSemanticCoreInstalled(): boolean {
    const resolve = (import.meta as ImportMeta & { resolve?: (specifier: string) => string }).resolve;
    if (typeof resolve !== "function") {
        return false;
    }
    try {
        return resolve(WEB_CORE_PACKAGE).length > 0;
    } catch {
        return false;
    }
}
