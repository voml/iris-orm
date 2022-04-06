/**
 * `@yydb/iris/node` — Node N-API facade, project helpers, and CLI builder.
 *
 * Import this entry from servers, SSR, tests, and Node tooling only.
 * Browser code must use the default `@yydb/iris` entry or `@yydb/iris/wasm`.
 */

export type { IrisBindings, IrisNodeBindings } from "../bindings.ts";
export { loadIrisNode, loadIrisNative, isNodeSemanticCoreInstalled } from "./load.ts";

export { checkSchemaFile } from "./check.ts";
export { createIris, type CreateIrisNodeOptions } from "./create.ts";
export { createIrisExecutor, createIrisDbBinding, createIrisBindingHost } from "./executor.ts";
export { printDoctorReport } from "./doctor.ts";
export {
    CONFIG_FILE_NAMES,
    LEGACY_CONFIG_FILE,
    RUNTIME_PROJECT_FILE,
    findAuthoringConfig,
    loadAuthoringConfig,
    resolveProjectRoot,
} from "./config.ts";
export { loadIrisConfig, loadProject, readProjectSchema, resolveProjectConfigPath } from "./project.ts";
export { openDatasourceSession } from "./datasource-session.ts";
export { createIrisTooling } from "./tooling.ts";
export { irisCoreVersion, packageVersion } from "./versions.ts";
