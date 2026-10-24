/**
 * `@yydb/iris` default export — project config helpers for browser / Worker graphs.
 *
 * The browser semantic core (WASM) lives in `@yydb/iris-language`.
 * Protocol types: `@yydb/iris/types`. Node apps use `@yydb/iris/node`.
 */

export { defineConfig, toProjectDocument } from "../types/config.ts";
export type { IrisUserConfig, IrisProjectDocument } from "../types/config.ts";
