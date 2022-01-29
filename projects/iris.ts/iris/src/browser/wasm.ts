import { IrisFacadeError } from "../types/errors.ts";

const WASM_PACKAGE = "@yydb/iris-unknown-wasm32";

/** Explicit WASM bytes or module handle for hosts with custom asset pipelines. */
export type WasmSource = URL | Request | Response | ArrayBuffer | Uint8Array | WebAssembly.Module;

export type InitIrisOptions = {
    /** When omitted, the default optional platform package asset is used. */
    module?: WasmSource;
};

type WasmBinding = {
    initWasm(options?: { module?: WasmSource }): Promise<void>;
    irisVersion(): string;
    checkSource(source: string): {
        ok: boolean;
        tableCount: number;
        schemaFingerprint: string;
        generatorVersion: string;
        error?: string | null;
    };
    resetWasmBindingForTests(): void;
};

let initDone = false;
let wasmBinding: WasmBinding | null = null;

async function loadWasmBinding(): Promise<WasmBinding> {
    try {
        return (await import(WASM_PACKAGE)) as WasmBinding;
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
            `@yydb/iris: install optional dependency ${WASM_PACKAGE} (same version as @yydb/iris): ${message}`,
        );
    }

    await binding.initWasm({ module: options.module });
    wasmBinding = binding;
    initDone = true;
}

/** @internal */
export function assertWasmInitialized(): void {
    if (!initDone || wasmBinding == null) {
        throw new IrisFacadeError("wasm-not-initialized", "@yydb/iris: call initIris() before createIris()");
    }
}

/** @internal Exposed for binding tests. */
export function getWasmBindingForTests(): WasmBinding | null {
    return wasmBinding;
}

/** @internal */
export function resetWasmStateForTests(): void {
    initDone = false;
    wasmBinding?.resetWasmBindingForTests();
    wasmBinding = null;
}
