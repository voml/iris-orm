import type {
    WebIrisDiagnostic,
    WebOperationExecutor,
    WebOperationRequest,
} from "@yydb/iris-unknown-wasm32";

/** Validate operation identity against the expected build-time contract fingerprint. */
export function validateContractFingerprint(
    request: WebOperationRequest,
    expectedFingerprint: string,
): WebIrisDiagnostic | null {
    if (request.identity.contractFingerprint !== expectedFingerprint) {
        return {
            code: "IRIS-CONTRACT-MISMATCH",
            message: `contract fingerprint mismatch (expected ${expectedFingerprint}, got ${request.identity.contractFingerprint})`,
            severity: "error",
        };
    }
    return null;
}

function rejectEnvelope(diagnostic: WebIrisDiagnostic) {
    return { ok: false as const, diagnostics: [diagnostic] };
}

/** Wrap an executor so every request must match the bound contract fingerprint. */
export function guardOperationExecutor(executor: WebOperationExecutor, expectedFingerprint: string): WebOperationExecutor {
    return {
        async execute(request: WebOperationRequest) {
            const diagnostic = validateContractFingerprint(request, expectedFingerprint);
            if (diagnostic) {
                return rejectEnvelope(diagnostic);
            }
            return executor.execute(request);
        },
        async executeUnit(request: WebOperationRequest) {
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
