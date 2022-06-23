/**
 * `@yydb/iris/cloudflare` — Cloudflare Worker + D1 facade.
 *
 * Not included in the generic browser entry. Generated Worker clients import this
 * entry for D1-backed `OperationExecutor` wiring.
 */

export { createIrisOperationExecutor, type CreateIrisCloudflareExecutorOptions } from "./executor.ts";
export { createD1ReadOperationExecutor } from "./d1-executor.ts";
export {
    bindD1Parameters,
    coerceD1BindValue,
    resolveD1Plan,
    resolveD1PlanForRequest,
    type D1PhysicalPlan,
    type D1PlanRegistry,
} from "./d1-plan.ts";
export type { IrisD1BoundStatement, IrisD1Database, IrisD1PreparedStatement } from "./types.ts";
