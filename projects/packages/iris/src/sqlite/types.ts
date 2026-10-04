/** Minimal SQLite prepare/bind surface shared by D1 and other SQLite-shaped adapters. */
export interface IrisSqlitePreparedStatement {
    bind(...values: unknown[]): IrisSqliteBoundStatement;
}

export interface IrisSqliteBoundStatement {
    all<T = Record<string, unknown>>(): Promise<{ results: T[] }>;
    run(): Promise<{ success: boolean }>;
}

export interface IrisSqliteDatabase {
    prepare(query: string): IrisSqlitePreparedStatement;
}
