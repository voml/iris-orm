import type { IrisDiagnostic, OperationRequest } from "../types/contract.ts";
import type { OperationExecutor } from "../types/operation-executor.ts";

/** Validate operation identity against the expected build-time contract fingerprint. */
export function validateContractFingerprint(
    request: OperationRequest,
    expectedFingerprint: string,
): IrisDiagnostic | null {
    if (request.identity.contractFingerprint !== expectedFingerprint) {
        return {
            code: "IRIS-CONTRACT-MISMATCH",
            message: `contract fingerprint mismatch (expected ${expectedFingerprint}, got ${request.identity.contractFingerprint})`,
            severity: "error",
        };
    }
    return null;
}

function rejectEnvelope(diagnostic: IrisDiagnostic) {
    return { ok: false as const, diagnostics: [diagnostic] };
}

/** Wrap an executor so every request must match the bound contract fingerprint. */
export function guardOperationExecutor(executor: OperationExecutor, expectedFingerprint: string): OperationExecutor {
    return {
        async execute(request: OperationRequest) {
            const diagnostic = validateContractFingerprint(request, expectedFingerprint);
            if (diagnostic) {
                return rejectEnvelope(diagnostic);
            }
            return executor.execute(request);
        },
        async executeUnit(request: OperationRequest) {
            const diagnostic = validateContractFingerprint(request, expectedFingerprint);
            if (diagnostic) {
                return rejectEnvelope(diagnostic);
            }
            return executor.executeUnit(request);
        },
        close() {
            return executor.close();
        },
    };
}
