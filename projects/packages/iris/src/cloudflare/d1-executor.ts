import type { OperationRequest, ResultEnvelope } from "../types/contract.ts";

import type { ExecutionRow } from "../types/execution-result.ts";

import type { OperationExecutor } from "../types/operation-executor.ts";

import type { D1PlanRegistry } from "./d1-plan.ts";

import { createSqlitePlanOperationExecutor } from "../sqlite/sqlite-plan-executor.ts";

import type { IrisSqliteDatabase } from "../sqlite/types.ts";

import type { IrisD1Database } from "./types.ts";



/**

 * D1 operation executor driven by build-time plan artifacts.

 *

 * Read and `write-returning` plans run on `execute`. Plain `write` plans run on `executeUnit`.

 */

export function createD1OperationExecutor(

    d1: IrisD1Database,

    plans: D1PlanRegistry = {},

    wireNamesByEntity: Readonly<Record<string, Readonly<Record<string, string>>>> = {},

): OperationExecutor {

    return createSqlitePlanOperationExecutor(d1 as IrisSqliteDatabase, plans, wireNamesByEntity);

}



/** @deprecated Use `createD1OperationExecutor`. */

export const createD1ReadOperationExecutor = createD1OperationExecutor;


