/**
 * `@yydb/iris` default export — browser / Worker facade (WASM inside).
 *
 * `defineConfig` for `iris.config.ts` is re-exported here for tooling ergonomics.
 * Protocol types: `@yydb/iris/types`. Node apps use `@yydb/iris/node`.
 * Low-level WASM loader: `@yydb/iris/wasm`.
 */

export { defineConfig, toProjectDocument } from "../types/config.ts";
export type { IrisUserConfig, IrisProjectDocument } from "../types/config.ts";
export { createIris, type CreateIrisBrowserOptions } from "./create.ts";
export { createIrisDbBinding, createBrowserIrisDbBinding, createIrisOperationExecutor } from "./executor.ts";
export { initIris, loadIrisWeb, type InitIrisOptions, type WasmSource } from "../wasm/index.ts";
export { openLocalStore, type LocalStore, type LocalStoreBackend, type OpenLocalStoreOptions } from "./local-store.ts";
