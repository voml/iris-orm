/**
 * Web runtime helpers for `@yydb/iris-language`.
 *
 * These modules are owned by the browser line and must not import the desktop
 * N-API package. WASM loader entry points stay in `../wasm.ts`.
 */

export { buildRuntime } from "./build-runtime.ts";
export { openBindingSession, resolveBindingProfile } from "./open-binding-session.ts";
export { createIrisDbBindingFromSession } from "./db-binding.ts";
export { createOperationExecutorFromSession } from "./operation-binding.ts";
export { guardOperationExecutor, validateContractFingerprint } from "./validate-contract.ts";
