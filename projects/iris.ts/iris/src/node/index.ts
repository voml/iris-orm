/**
 * `@yydb/iris/node` — Node N-API facade, project helpers, and CLI builder.
 *
 * Import this entry from servers, SSR, tests, and Node tooling only.
 * Browser code must use the default `@yydb/iris` entry.
 */

export { checkSchemaFile } from "./check.ts";
export { createIris, type CreateIrisNodeOptions } from "./create.ts";
export { createIrisCli } from "./create-cli.ts";
export { printDoctorReport } from "./doctor.ts";
export { loadNativeBinding, resolvePlatformPackageName, resetNativeBindingCacheForTests, type NativeBinding } from "./native.ts";
export type { IrisNativeCheckResult, IrisNativeModule } from "./native-module.ts";
export { loadProject } from "./project.ts";
