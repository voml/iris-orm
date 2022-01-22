/**
 * `@yydb/iris` default export — browser / Worker Web facade (WASM inside).
 *
 * Protocol types: `@yydb/iris/types`. Node apps use `@yydb/iris/node`.
 */

export { createIris, type CreateIrisWebOptions } from "./create.ts";
export { initIris, type InitIrisOptions, type WasmSource } from "./wasm.ts";
