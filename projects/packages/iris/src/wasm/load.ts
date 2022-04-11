import type { CheckSourceResult } from "../types/check-source.ts";
import { IrisFacadeError } from "../types/errors.ts";
import type { IrisBindings, IrisWasmOptions, MemorySessionBinding, WasmSource } from "../bindings.ts";

const WEB_CORE_PACKAGE = "@yydb/iris-unknown-wasm32";

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

async function importWasmPackage(): Promise<WasmPackage> {
    try {
        return (await import(WEB_CORE_PACKAGE)) as WasmPackage;
    } catch {
        const workspaceEntry = new URL("../../../iris-unknown-wasm32/src/index.ts", import.meta.url);
        return (await import(workspaceEntry.href)) as WasmPackage;
    }
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
            `@yydb/iris: browser semantic core not installed (reinstall @yydb/iris with optional dependencies): ${message}`,
        );
    }

    try {
        await pkg.initWasm({ module: options.module });
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        throw new IrisFacadeError(
            "wasm-package-missing",
            `@yydb/iris: browser semantic core not built (run wasm build): ${message}`,
        );
    }
    return toBindings(pkg);
}
