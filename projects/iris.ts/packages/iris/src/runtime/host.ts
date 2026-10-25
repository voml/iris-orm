/**
 * Host type surface for the shared Iris runtime modules.
 *
 * Every other module under `runtime/` is generated from the `iris-generator`
 * templates, so this adapter is the only place that names concrete desktop
 * contract types. Keep it to re-exports.
 */

export type { IrisDiagnostic, OperationRequest, ResultEnvelope } from "../types/contract.ts";
export type { ExecutionRow, ExecutionWireResult, WireCarrierValue } from "../types/execution-result.ts";
export { mapWireToQueryValue } from "../types/executor.ts";
export type { IrisDbBinding, VosParameters } from "../types/executor.ts";
export type { OperationExecutor } from "../types/operation-executor.ts";
export type { SchemaIntrospection } from "../types/schema-introspection.ts";
export type { ExecuteResult, IrisRow } from "../types/session.ts";
export type { MemorySessionBinding, SessionExecuteWire } from "../bindings.ts";
