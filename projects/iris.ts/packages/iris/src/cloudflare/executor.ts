import type { CreateIrisDbBindingOptions } from "../types/executor.ts";
import type { OperationExecutor } from "../types/operation-executor.ts";

import type { IrisD1Database } from "./types.ts";

/** Cloudflare Worker wiring for generated clients (`profile: d1`). */
export interface CreateIrisCloudflareExecutorOptions extends CreateIrisDbBindingOptions {
    /** Bound D1 database from the Worker environment. */
    d1: IrisD1Database;
    /** Build-time physical plan artifact from generated `_internal/d1-plans.ts`. */
    plans?: import("./d1-plan.ts").D1PlanRegistry;
    /** Wire-name map from generated `metadata.ts` (`IRIS_FIELD_WIRE_NAMES`). */
    wireNamesByEntity?: Readonly<Record < string, Readonly < Record < string, string >>> >;
    /** VOS value-carrier map from generated `metadata.ts` (`IRIS_VALUE_CARRIERS`). */
    valueCarriersByEntity?: Readonly<Record < string, Readonly < Record < string, string >>> >;
}

/**
 * P0 2026-10-27 — **DEPRECATED**
 *
 * The Cloudflare Worker + D1 executor route is frozen / abandoned. The
 * long-lived Edge runtime contract is tracked in `06-d1-adapter-and-edge-host.md`
 * (root design repo). Types remain available but execution **throws**.
 *
 * `cloudflare-worker` as a host now uses `hostSupportsStorage` gating against
 * the explicitly chosen `IrisStorageProfile`. D1 happens to be supported as a
 * storage value on that host, but the executor binding below is not the
 * adapter we will ship.
 */
export async function createIrisOperationExecutor(
    _options: CreateIrisCloudflareExecutorOptions,
): Promise<OperationExecutor> {
    throw new Error(
        "@yydb/iris/cloudflare: D1 executor route is deprecated (P0) — see 06-d1-adapter-and-edge-host.md for the new Edge storage adapter contract",
    );
}
