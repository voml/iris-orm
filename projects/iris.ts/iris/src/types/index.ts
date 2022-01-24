/**
 * `@yydb/iris/types` — protocol, binding DTOs, VOS check helpers.
 *
 * No N-API loader, WASM loader, or CLI. Safe for adapters and codegen.
 */

export { IrisFacadeError } from "./errors.ts";
export type { IrisCapabilities, IrisHost, IrisRuntime } from "./protocol.ts";
export type { IrisPlaceholder } from "./placeholder.ts";
export { version } from "./version.ts";

export { checkSource } from "@game-gpt/vos-parser";
export type { VosCheckResult, VosDiagnostic } from "@game-gpt/vos-parser";
