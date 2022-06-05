import type { IrisBindings, IrisWasmOptions, WasmSource } from "../bindings.ts";
import { loadIrisWasm } from "./load.ts";
import { getCachedLoader, setActiveBindings, setCachedLoader } from "./state.ts";

export type { IrisBindings, IrisWasmOptions, WasmSource } from "../bindings.ts";
/** @deprecated Use `IrisWasmOptions`. */
export type InitIrisOptions = IrisWasmOptions;
export { loadIrisWasm } from "./load.ts";
export { createIrisDbBinding, createIrisOperationExecutor } from "./executor.ts";
export { getWasmSemanticCore, resetInitStateForTests } from "./state.ts";
export { isBrowserSemanticCoreInstalled } from "./install-check.ts";

/** Cached WASM binding loader (panduck `loadPanduckWeb` shape). */
export function loadIrisWeb(options: IrisWasmOptions = {}): Promise<IrisBindings> {
    const existing = getCachedLoader();
    if (existing) {
        return existing;
    }
    const loader = loadIrisWasm(options).then((bindings) => {
        setActiveBindings(bindings);
        return bindings;
    });
    setCachedLoader(loader);
    return loader;
}

/** @deprecated Prefer `loadIrisWeb`. One-time WASM init before `createIris` on browser hosts. */
export async function initIris(options: IrisWasmOptions = {}): Promise<void> {
    await loadIrisWeb(options);
}
