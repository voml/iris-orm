/**
 * `@yydb/iris/types` — protocol and binding DTOs (no loaders / CLI).
 */

export type { IrisPlaceholder } from "./placeholder.ts";
export type { IrisCapabilities, IrisHost, IrisRuntime } from "./protocol.ts";
export { version } from "./version.ts";

export { checkSource } from "@game-gpt/vos-parser";
export type { VosCheckResult, VosDiagnostic } from "@game-gpt/vos-parser";
