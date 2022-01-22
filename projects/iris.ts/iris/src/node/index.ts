/**
 * `@yydb/iris/node` — Node N-API facade + CLI-only APIs.
 *
 * Protocol types: `@yydb/iris/types`.
 */

export { createIris, loadProject, type CreateIrisNodeOptions } from "./create.ts";
export { createIrisCli } from "./create-cli.ts";
export { loadNativeBinding, resolvePlatformPackageName, type NativeBinding } from "./native.ts";
