import type { IrisBindings, IrisWasmOptions, WasmSource } from "../bindings.ts";
import { IrisFacadeError } from "../types/errors.ts";
import { loadIrisWasm } from "./load.ts";

export type { IrisBindings, IrisWasmOptions, WasmSource } from "../bindings.ts";
/** @deprecated Use `IrisWasmOptions`. */
export type InitIrisOptions = IrisWasmOptions;
export { loadIrisWasm, isBrowserSemanticCoreInstalled } from "./load.ts";

let cached: Promise<IrisBindings> | undefined;
let active: IrisBindings | null = null;

/** Cached WASM binding loader (panduck `loadPanduckWeb` shape). */
export function loadIrisWeb(options: IrisWasmOptions = {}): Promise<IrisBindings> {
    if (!cached) {
        cached = loadIrisWasm(options).then((bindings) => {
            active = bindings;
            return bindings;
        });
    }
    return cached;
}

/** @deprecated Prefer `loadIrisWeb`. One-time WASM init before `createIris` on browser hosts. */
export async function initIris(options: IrisWasmOptions = {}): Promise<void> {
    await loadIrisWeb(options);
}

/** Active semantic core after `loadIrisWeb()` / `initIris()`. */
export function getWasmSemanticCore(): IrisBindings {
    if (active == null) {
        throw new IrisFacadeError("wasm-not-initialized", "@yydb/iris: call initIris() or loadIrisWeb() before createIris()");
    }
    return active;
}

/** @internal Reset init gate (binding tests only). */
export async function resetInitStateForTests(): Promise<void> {
    cached = undefined;
    active = null;
    try {
        const pkg = await import("@yydb/iris-unknown-wasm32");
        pkg.resetWasmBindingForTests?.();
    } catch {
        const workspaceEntry = new URL("../../../iris-unknown-wasm32/src/index.ts", import.meta.url);
        const pkg = await import(workspaceEntry.href);
        pkg.resetWasmBindingForTests?.();
    }
}
