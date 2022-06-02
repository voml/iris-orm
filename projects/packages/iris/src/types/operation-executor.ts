import type { OperationRequest, ResultEnvelope } from "./contract.ts";
import type { ExecutionRow } from "./execution-result.ts";

/**
 * Async operation execution contract (target runtime ABI).
 *
 * Generated clients should prefer this over source-string `query` / `execute`.
 */
export interface OperationExecutor {
    execute(request: OperationRequest): Promise<ResultEnvelope<readonly ExecutionRow[]>>;
    /** Unit-valued / DDL-shaped declared operations. */
    executeUnit(request: OperationRequest): Promise<ResultEnvelope<void>>;
    close(): Promise<void>;
}
