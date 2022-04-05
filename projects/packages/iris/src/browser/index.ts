/**
 * `@yydb/iris` default export — browser / Worker facade (WASM inside).
 *
 * Protocol types: `@yydb/iris/types`. Node apps use `@yydb/iris/node`.
 * Low-level WASM loader: `@yydb/iris/wasm`.
 */

export { createIris, type CreateIrisBrowserOptions } from "./create.ts";
export { createBrowserIrisDbBinding } from "./executor.ts";
export { initIris, loadIrisWeb, type InitIrisOptions, type WasmSource } from "../wasm/index.ts";
export { openLocalStore, type LocalStore, type LocalStoreBackend, type OpenLocalStoreOptions } from "./local-store.ts";
