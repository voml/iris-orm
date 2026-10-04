import type { IrisOperation } from "./operation.ts";

/** Stable operation identity within a resolved contract. */
export interface OperationIdentity {
    readonly operationId: string;
    readonly contractFingerprint: string;
}

/** Host-neutral operation request (generated client primary ABI). */
export interface OperationRequest {
    readonly identity: OperationIdentity;
    readonly operation: IrisOperation;
    readonly parameters?: Readonly<Record<string, unknown>>;
    readonly deadlineMs?: number;
}

export type IrisDiagnosticSeverity = "error" | "warning";

/** Structured diagnostic returned with results or failures. */
export interface IrisDiagnostic {
    readonly code: string;
    readonly message: string;
    readonly severity: IrisDiagnosticSeverity;
    readonly retryable?: boolean;
    readonly unknownCommit?: boolean;
}

/** Build-time contract manifest fingerprint (wire subset). */
export interface ResolvedContract {
    readonly fingerprint: string;
    readonly schemaVersion: string;
    readonly operations: readonly string[];
}

/** Typed execution envelope for async `OperationExecutor`. */
export type ResultEnvelope<T = unknown> =
    | {
          ok: true;
          value: T;
          diagnostics?: readonly IrisDiagnostic[];
      }
    | {
          ok: false;
          diagnostics: readonly IrisDiagnostic[];
      };
