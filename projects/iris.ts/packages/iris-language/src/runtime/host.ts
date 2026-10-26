/**
 * Host type surface for the shared Iris runtime modules.
 *
 * Every other module under `runtime/` is generated from the `iris-generator`
 * templates, so this adapter is the only place that names the Web (WASM)
 * contract. Keep it to re-exports plus the wire-value mapping the shared
 * `db-binding` module needs.
 */

import type {
    WebExecuteResult,
    WebExecutionRow,
    WebExecutionWireResult,
    WebIrisDbBinding,
    WebIrisDiagnostic,
    WebIrisRow,
    WebMemorySessionBinding,
    WebOperationExecutor,
    WebOperationRequest,
    WebResultEnvelope,
    WebSchemaIntrospection,
    WebSessionExecuteWire,
    WebVosParameters,
    WebWireCarrierValue,
} from "../web-contract.ts";

export type {
    WebExecuteResult as ExecuteResult,
    WebExecutionRow as ExecutionRow,
    WebExecutionWireResult as ExecutionWireResult,
    WebIrisDbBinding as IrisDbBinding,
    WebIrisDiagnostic as IrisDiagnostic,
    WebIrisRow as IrisRow,
    WebMemorySessionBinding as MemorySessionBinding,
    WebOperationExecutor as OperationExecutor,
    WebOperationRequest as OperationRequest,
    WebResultEnvelope as ResultEnvelope,
    WebSchemaIntrospection as SchemaIntrospection,
    WebSessionExecuteWire as SessionExecuteWire,
    WebVosParameters as VosParameters,
    WebWireCarrierValue as WireCarrierValue,
};

/** Map binding wire shape to the VOS value surfaced by `db.$query`. */
export function mapWireToQueryValue(wire: WebExecutionWireResult): unknown {
    switch (wire.kind) {
        case "rows":
            return wire.rows;
        case "value":
            return wire.value;
        case "affected":
            return wire.affected;
        case "unit":
            return undefined;
    }
}
