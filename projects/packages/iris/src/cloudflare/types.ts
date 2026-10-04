import type {

    IrisSqliteBoundStatement,

    IrisSqliteDatabase,

    IrisSqlitePreparedStatement,

} from "../sqlite/types.ts";



/** Minimal D1 surface for Iris Cloudflare adapters (no Workers type package dependency). */

export type IrisD1PreparedStatement = IrisSqlitePreparedStatement;



export type IrisD1BoundStatement = IrisSqliteBoundStatement;



export type IrisD1Database = IrisSqliteDatabase;


