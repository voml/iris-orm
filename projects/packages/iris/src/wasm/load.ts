import { createRequire } from "node:module";

import type { CheckSourceResult } from "../types/check-source.ts";
import { IrisFacadeError } from "../types/errors.ts";
import type { IrisBindings, IrisWasmOptions, MemorySessionBinding, WasmSource } from "../bindings.ts";

const WEB_CORE_PACKAGE = "@yydb/iris-unknown-wasm32";
const require = createRequire(import.meta.url);

/** Whether the optional browser semantic core package resolves. */
export function isBrowserSemanticCoreInstalled(): boolean {
    try {
        require.resolve(WEB_CORE_PACKAGE);
        return true;
    } catch {
        return false;
    }
}

type WasmPackage = {
    initWasm(options?: { module?: WasmSource }): Promise<void>;
    irisVersion(): string;
    checkSource(source: string): CheckSourceResult;
    introspectSchema(source: string): string;
    openMemorySession(): WasmMemorySession;
    resetWasmBindingForTests(): void;
};

type WasmMemorySession = {
    executeVos(source: string): string;
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
    return {
        executeVos: (source: string) => session.executeVos(source),
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

/** Load and initialize the browser WASM semantic core. */
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

    await pkg.initWasm({ module: options.module });
    return toBindings(pkg);
}
