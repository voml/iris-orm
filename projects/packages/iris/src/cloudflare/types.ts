/** Minimal D1 surface for Iris Cloudflare adapters (no Workers type package dependency). */
export interface IrisD1PreparedStatement {
    bind(...values: unknown[]): IrisD1BoundStatement;
}

export interface IrisD1BoundStatement {
    all<T = Record<string, unknown>>(): Promise<{ results: T[] }>;
    run(): Promise<{ success: boolean }>;
}

export interface IrisD1Database {
    prepare(query: string): IrisD1PreparedStatement;
}
