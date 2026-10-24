import type { OperationIdentity, OperationRequest } from "../types/contract.ts";
import type { IrisOperation } from "../types/operation.ts";

/** Build a host-neutral operation request for `OperationExecutor`. */
export function buildOperationRequest(
    identity: OperationIdentity,
    operation: IrisOperation,
    parameters?: Readonly<Record<string, unknown>>,
    deadlineMs?: number,
): OperationRequest {
    return {
        identity,
        operation,
        parameters,
        deadlineMs,
    };
}

/** Declared VOS operation synthesized at build time by generated clients. */
export function declaredVosOperation(source: string): IrisOperation {
    return { kind: "declared-vos", source };
}
