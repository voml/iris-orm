/**
 * Cloudflare Worker wiring for generated clients (`profile: d1`).
 *
 * Thin facade over `createD1OperationExecutor` from `./d1-executor.ts`.
 * See that module for the WASM round-trip contract.
 */
import type { CreateIrisDbBindingOptions } from "../types/executor.ts";
import type { OperationExecutor } from "../types/operation-executor.ts";

import { createD1OperationExecutor } from "./d1-executor.ts";
import type { IrisD1Database } from "./types.ts";

export interface CreateIrisCloudflareExecutorOptions extends CreateIrisDbBindingOptions {
    /** Bound D1 database from the Worker environment (`env.DB`). */
    d1: IrisD1Database;
    /** Build-time physical plan artifact from generated `_internal/d1-plans.ts`. */
    plans?: import("./d1-plan.ts").D1PlanRegistry;
    /** Wire-name map from generated `metadata.ts` (`IRIS_FIELD_WIRE_NAMES`). */
    wireNamesByEntity?: Readonly<Record < string, Readonly < Record < string, string >>> >;
    /** VOS value-carrier map from generated `metadata.ts` (`IRIS_VALUE_CARRIERS`). */
    valueCarriersByEntity?: Readonly<Record < string, Readonly < Record < string, string >>> >;
}

/**
 * Create the async D1-backed `OperationExecutor` generated Worker clients
 * consume.
 *
 * Replaces the previous runtime-throw stub (P0 2026-10-27).  The executor
 * routes every call through the WASM `prepareD1Operation` → D1 execute →
 * `decodeD1Result` bridge in `@yydb/iris-language/cloudflare/d1-wasm-bridge`.
 */
export async function createIrisOperationExecutor(
    options: CreateIrisCloudflareExecutorOptions,
): Promise<OperationExecutor> {
    return createD1OperationExecutor(
        options.d1,
        options.plans,
        options.wireNamesByEntity,
        options.valueCarriersByEntity,
    );
}
