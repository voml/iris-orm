/**
 * `@yydb/iris/node` — Node N-API facade, project helpers, and CLI builder.
 *
 * Import this entry from servers, SSR, tests, and Node tooling only.
 * Browser code must use the default `@yydb/iris` entry.
 */

export { checkSchemaFile } from "./check.ts";
export { createIris, type CreateIrisNodeOptions } from "./create.ts";
export { createIrisCli } from "./create-cli.ts";
export { createIrisExecutor, createIrisDbBinding, createIrisBindingHost } from "./executor.ts";
export { generateTypescriptClient } from "./generate-typescript.ts";
export { printDoctorReport } from "./doctor.ts";
export { loadProject, readProjectSchema } from "./project.ts";
export { openDatasourceSession } from "./datasource-session.ts";
export { createIrisTooling } from "./tooling.ts";
