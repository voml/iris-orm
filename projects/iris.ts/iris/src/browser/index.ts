/**
 * `@yydb/iris` default export — browser / Worker facade (WASM inside).
 *
 * Protocol types: `@yydb/iris/types`. Node apps use `@yydb/iris/node`.
 * There is no `@yydb/iris/web` public subpath.
 */

export { createIris, type CreateIrisBrowserOptions } from "./create.ts";
export { initIris, type InitIrisOptions, type WasmSource } from "./wasm.ts";
export { openLocalStore, type LocalStore, type LocalStoreBackend, type OpenLocalStoreOptions } from "./local-store.ts";
