import type { CreateIrisDbBindingOptions } from "../types/executor.ts";
import type { OperationExecutor } from "../types/operation-executor.ts";
import { guardOperationExecutor } from "../runtime/validate-contract.ts";
import { createD1ReadOperationExecutor } from "./d1-executor.ts";
import type { IrisD1Database } from "./types.ts";

/** Cloudflare Worker wiring for generated clients (`profile: d1`). */
export interface CreateIrisCloudflareExecutorOptions extends CreateIrisDbBindingOptions {
    /** Bound D1 database from the Worker environment. */
    d1: IrisD1Database;
}

/** Create the async operation executor for generated Cloudflare clients. */
export async function createIrisOperationExecutor(
    options: CreateIrisCloudflareExecutorOptions,
): Promise<OperationExecutor> {
    if (!options.d1) {
        throw new Error("@yydb/iris/cloudflare: d1 binding is required");
    }
    const executor = createD1ReadOperationExecutor(options.d1);
    if (options.contractFingerprint) {
        return guardOperationExecutor(executor, options.contractFingerprint);
    }
    return executor;
}
