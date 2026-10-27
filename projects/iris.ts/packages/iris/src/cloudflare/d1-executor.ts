import type { OperationRequest, ResultEnvelope } from "../types/contract.ts";

import type { ExecutionRow } from "../types/execution-result.ts";

import type { OperationExecutor } from "../types/operation-executor.ts";

import type { D1PlanRegistry } from "./d1-plan.ts";

import { createSqlitePlanOperationExecutor } from "../sqlite/sqlite-plan-executor.ts";

import type { IrisSqliteDatabase } from "../sqlite/types.ts";

import type { IrisD1Database } from "./types.ts";

/**
 * P0 2026-10-27 — **DEPRECATED**
 *
 * The Cloudflare Worker + D1 executor route is frozen / abandoned. The
 * long-lived Edge runtime contract is tracked in `06-d1-adapter-and-edge-host.md`
 * (root design repo). This function still **compiles** (types remain
 * available) but throws at runtime to prevent accidental use.
 *
 * A future adapter (`yyds` / `remote-http` storage) will replace this path.
 */
export function createD1OperationExecutor(

    _d1: IrisD1Database,

    _plans: D1PlanRegistry = {},

    _wireNamesByEntity: Readonly<Record < string, Readonly < Record < string, string >>> > = {},

    _valueCarriersByEntity: Readonly<Record < string, Readonly < Record < string, string >>> > = {},

): OperationExecutor {

    throw new Error(

        "@yydb/iris/cloudflare: D1 executor route is deprecated (P0) — see 06-d1-adapter-and-edge-host.md for the new Edge storage adapter contract",

    );

}



/** @deprecated Use `createD1OperationExecutor`. */

export const createD1ReadOperationExecutor = createD1OperationExecutor;
