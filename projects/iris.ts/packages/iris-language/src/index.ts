/**
 * `@yydb/iris-language` — browser / Worker language layer on the WASM semantic core.
 *
 * Loads `@yydb/iris-unknown-wasm32`, caches the active bindings, and exposes the
 * symmetric binding factories consumed by generated `browser.ts`.
 */

export {
    getWasmSemanticCore,
    initIris,
    isBrowserSemanticCoreInstalled,
    loadIrisWasm,
    loadIrisWeb,
    resetInitStateForTests,
} from "./wasm.ts";
export type { InitIrisOptions, IrisWasmOptions, WasmSource } from "./wasm.ts";

export {
    checkSource,
    executeVosMemory,
    initWasm,
    introspectSchema,
    irisVersion,
    openMemorySession,
} from "@yydb/iris-unknown-wasm32";
export type { CheckSourceResult } from "./web-contract.ts";
export type { WebCreateIrisDbBindingOptions as CreateIrisDbBindingOptions } from "./web-contract.ts";

export {
    createBrowserIrisDbBinding,
    createIris,
    createIrisDbBinding,
    createIrisOperationExecutor,
    openLocalStore,
} from "./executor.ts";
export type {
    CreateIrisBrowserOptions,
    LocalStore,
    LocalStoreBackend,
    OpenLocalStoreOptions,
} from "./executor.ts";
