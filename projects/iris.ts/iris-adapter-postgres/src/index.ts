/**
 * Isolated PostgreSQL foreign-store adapter (`@yydb/iris-adapter-postgres`).
 *
 * Driver choice: **postgres** (porsager/postgres.js) — pure JS wire protocol,
 * not `pg-native` / libpq bindings. SQL stays private to this package.
 */

import type { IrisPlaceholder } from "@yydb/iris/types";

/** Pairs with Rust `iris-adapter-postgres`. */
export const BACKEND_ID = "postgres" as const;

export type PostgresAdapterOptions = {
    /** postgres.js connection URL or options object (typed when peer is installed). */
    url: string;
};

/** Placeholder source handle until catalog / execute / migrate land. */
export class PostgresSource {
    readonly backendId = BACKEND_ID;

    constructor(readonly options: PostgresAdapterOptions) {}

    placeholder(): IrisPlaceholder {
        return { __irisTypes: "rust-binding-skeleton" };
    }
}
