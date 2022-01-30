/**
 * `@yydb/iris/types` — protocol and binding DTOs.
 *
 * No N-API loader, WASM loader, or CLI. Semantic validation runs in Rust bindings only.
 */

export { IrisFacadeError } from "./errors.ts";
export type { IrisCapabilities, IrisHost, IrisRuntime } from "./protocol.ts";
export type { IrisPlaceholder } from "./placeholder.ts";
export type { CheckSourceResult } from "./check-source.ts";
export { version } from "./version.ts";
