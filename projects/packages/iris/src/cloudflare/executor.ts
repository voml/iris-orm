import type { CreateIrisDbBindingOptions } from "../types/executor.ts";
import type { OperationExecutor } from "../types/operation-executor.ts";
import { guardOperationExecutor } from "../runtime/validate-contract.ts";
import { createD1OperationExecutor } from "./d1-executor.ts";
import type { D1PlanRegistry } from "./d1-plan.ts";
import type { IrisD1Database } from "./types.ts";

/** Cloudflare Worker wiring for generated clients (`profile: d1`). */
export interface CreateIrisCloudflareExecutorOptions extends CreateIrisDbBindingOptions {
    /** Bound D1 database from the Worker environment. */
    d1: IrisD1Database;
    /** Build-time physical plan artifact from generated `_internal/d1-plans.ts`. */
    plans?: D1PlanRegistry;
    /** Wire-name map from generated `metadata.ts` (`IRIS_FIELD_WIRE_NAMES`). */
    wireNamesByEntity?: Readonly<Record<string, Readonly<Record<string, string>>>>;
}

/** Create the async operation executor for generated Cloudflare clients. */
export async function createIrisOperationExecutor(
    options: CreateIrisCloudflareExecutorOptions,
): Promise<OperationExecutor> {
    if (!options.d1) {
        throw new Error("@yydb/iris/cloudflare: d1 binding is required");
    }
    const executor = createD1OperationExecutor(options.d1, options.plans ?? {}, options.wireNamesByEntity ?? {});
    if (options.contractFingerprint) {
        return guardOperationExecutor(executor, options.contractFingerprint);
    }
    return executor;
}
