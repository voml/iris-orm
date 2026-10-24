/**
 * Host-agnostic Iris runtime helpers shared by Node and browser bindings.
 *
 * WASM / N-API loader entry points are intentionally not exported here.
 */

export { buildRuntime, buildRuntimeForSession } from "./build-runtime.ts";
export { openBindingSession, resolveBindingProfile } from "./open-binding-session.ts";
export type { BindingSessionHooks } from "./open-binding-session.ts";
export { createIrisDbBindingFromSession } from "./db-binding.ts";
export type { DbBindingSession } from "./db-binding.ts";
export { createOperationExecutorFromSession } from "./operation-binding.ts";
export type { OperationExecutorSessionOptions } from "./operation-binding.ts";
export { guardOperationExecutor, validateContractFingerprint } from "./validate-contract.ts";
export type { IrisBindings, MemorySessionBinding, OpenSessionNapiOptions } from "../bindings.ts";
