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
    /** Open the scope used by generated `Client.transaction`. */
    begin(): Promise<ResultEnvelope<void>>;
    /** Commit the active scope. */
    commit(): Promise<ResultEnvelope<void>>;
    /** Roll back the active scope. */
    rollback(): Promise<ResultEnvelope<void>>;
    close(): Promise<void>;
}
