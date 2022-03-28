/** Session profile for `openSession`. */
export type IrisSessionProfile = "memory" | "reference" | "sqlite" | "postgres" | "mysql" | "project";

/** Options for opening a symmetric Iris session. */
export interface OpenSessionOptions {
    /** Adapter profile (default `memory` on browser; Node may use foreign adapters). */
    profile?: IrisSessionProfile;
    /** Node: SQLite path or `:memory:`. */
    sqlitePath?: string;
    /** Node: PostgreSQL connection URL. */
    postgresUrl?: string;
    /** Node: MySQL connection URL. */
    mysqlUrl?: string;
    /** Node: project root or `iris.von` path (`profile=project`). */
    project?: string;
    /** Node: datasource name inside `iris.von` (default `default`). */
    source?: string;
}

/** One logical row returned from VOS execution. */
export type IrisRow = Record<string, string | number | boolean | null>;

/** Result of executing VOS source on a session. */
export interface ExecuteResult {
    ok: boolean;
    rows: IrisRow[];
    error?: string | null;
}

/** Symmetric session contract (browser + Node). */
export interface IrisSession {
    /** Plan and execute VOS source on the bound adapter. */
    execute(source: string): ExecuteResult;
    /** Structured operation ABI (generated client path). */
    executeOperation?(operation: import("./operation.ts").IrisOperation): ExecuteResult;
    /** Release session resources. */
    close(): void;
    /**
     * Debug / SQLite bootstrap only.
     * @deprecated Application schema changes use `iris push`, not session helpers.
     */
    managedPush?(schema: string): void;
}
