/**
 * `@yydb/iris/opfs` — browser OPFS + SQLite facade.
 *
 * Not included in the generic browser entry. Generated OPFS clients import this
 * entry for durable local-first operation execution.
 */

export { createIrisOperationExecutor, type CreateIrisOpfsExecutorOptions } from "./executor.ts";
export { openOpfsStore, type OpenOpfsStoreOptions, type OpfsStore } from "./opfs-store.ts";
export { createSqlitePlanOperationExecutor } from "../sqlite/sqlite-plan-executor.ts";
export type { IrisSqliteBoundStatement, IrisSqliteDatabase, IrisSqlitePreparedStatement } from "../sqlite/types.ts";
