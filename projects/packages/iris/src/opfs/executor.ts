import type { CreateIrisDbBindingOptions } from "../types/executor.ts";
import type { OperationExecutor } from "../types/operation-executor.ts";
import type { D1PlanRegistry } from "../cloudflare/d1-plan.ts";
import { guardOperationExecutor } from "../runtime/validate-contract.ts";
import { createSqlitePlanOperationExecutor } from "../sqlite/sqlite-plan-executor.ts";
import type { IrisSqliteDatabase } from "../sqlite/types.ts";

/** Browser OPFS wiring for generated clients (`profile: opfs`). */
export interface CreateIrisOpfsExecutorOptions extends CreateIrisDbBindingOptions {
    /** SQLite execution surface backed by an OPFS file (host-provided until `@yyds/sqlite` OPFS lands). */
    sqlite: IrisSqliteDatabase;
    /** Build-time physical plan artifact from generated `_internal/d1-plans.ts`. */
    plans?: D1PlanRegistry;
    /** Wire-name map from generated `metadata.ts` (`IRIS_FIELD_WIRE_NAMES`). */
    wireNamesByEntity?: Readonly<Record<string, Readonly<Record<string, string>>>>;
}

/** Create the async operation executor for generated browser OPFS clients. */
export async function createIrisOperationExecutor(
    options: CreateIrisOpfsExecutorOptions,
): Promise<OperationExecutor> {
    if (!options.sqlite) {
        throw new Error("@yydb/iris/opfs: sqlite handle is required");
    }
    const executor = createSqlitePlanOperationExecutor(
        options.sqlite,
        options.plans ?? {},
        options.wireNamesByEntity ?? {},
    );
    if (options.contractFingerprint) {
        return guardOperationExecutor(executor, options.contractFingerprint);
    }
    return executor;
}
