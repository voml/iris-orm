import type { CheckSourceResult } from "../types/check-source.ts";
import { IrisFacadeError } from "../types/errors.ts";

const WEB_CORE_PACKAGE = "@yydb/iris-unknown-wasm32";

/** Explicit WASM bytes or module handle for hosts with custom asset pipelines. */
export type WasmSource = URL | Request | Response | ArrayBuffer | Uint8Array | WebAssembly.Module;

export type InitIrisOptions = {
    /** When omitted, the default optional semantic core asset is used. */
    module?: WasmSource;
};

type WasmBinding = {
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

let initDone = false;
let wasmBinding: WasmBinding | null = null;

async function loadWasmBinding(): Promise<WasmBinding> {
    try {
        return (await import(WEB_CORE_PACKAGE)) as WasmBinding;
    } catch {
        const workspaceEntry = new URL("../../../iris-unknown-wasm32/src/index.ts", import.meta.url);
        return (await import(workspaceEntry.href)) as WasmBinding;
    }
}

/** One-time WASM init before `createIris` on browser hosts. */
export async function initIris(options: InitIrisOptions = {}): Promise<void> {
    if (initDone) {
        return;
    }

    let binding: WasmBinding;
    try {
        binding = await loadWasmBinding();
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        throw new IrisFacadeError(
            "wasm-package-missing",
            `@yydb/iris: browser semantic core not installed (reinstall @yydb/iris with optional dependencies): ${message}`,
        );
    }

    await binding.initWasm({ module: options.module });
    wasmBinding = binding;
    initDone = true;
}

export function assertWasmInitialized(): void {
    if (!initDone || wasmBinding == null) {
        throw new IrisFacadeError("wasm-not-initialized", "@yydb/iris: call initIris() before createIris()");
    }
}

/** Active semantic core after `initIris()`. */
export function getWasmSemanticCore(): WasmBinding {
    assertWasmInitialized();
    return wasmBinding as WasmBinding;
}

export function resetInitStateForTests(): void {
    initDone = false;
    wasmBinding?.resetWasmBindingForTests();
    wasmBinding = null;
}
