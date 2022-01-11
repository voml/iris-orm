/**
 * `@yydb/iris` — public TypeScript facade + `iris` CLI (pairs with Rust `iris::*`).
 *
 * Applications depend on this package. Foreign-store adapters are separate
 * packages (`@yydb/iris-adapter-*`). Native connectors land as
 * `@yydb/iris-connector-*` when ready.
 *
 * VOS language: `@game-gpt/vos-parser` (+ transitive `@game-gpt/vos-ast`).
 * Do not rebuild VOS; do not use `@yydb/vos` as authority.
 */

export type { IrisPlaceholder } from "@yydb/iris-types";
export { checkSource } from "@game-gpt/vos-parser";
export type { VosCheckResult, VosDiagnostic } from "@game-gpt/vos-parser";

export { createIrisCli } from "./create-cli.ts";

/** Package identity for install smoke. */
export const version = "0.1.0";
