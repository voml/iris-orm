/**
 * Isolated SQLite foreign-store adapter (`@yydb/iris-adapter-sqlite`).
 *
 * Driver choice: **sql.js** (WASM) — not `better-sqlite3` / node native bindings.
 * Backend commands stay private to this package; never expose SQL on the Iris facade.
 */

import type { IrisPlaceholder } from "@yydb/iris/types";

/** Pairs with Rust `iris-adapter-sqlite`. */
export const BACKEND_ID = "sqlite" as const;

export type SqliteAdapterOptions = {
    /** File path or `:memory:`. */
    path: string;
};

/**
 * Placeholder source handle. Catalog / execute / migrate land later and will
 * dynamically import `sql.js` so installs without the peer still typecheck.
 */
export class SqliteSource {
    readonly backendId = BACKEND_ID;

    constructor(readonly options: SqliteAdapterOptions) {}

    /** Smoke hook until real open lands. */
    placeholder(): IrisPlaceholder {
        return { __irisTypes: "rust-binding-skeleton" };
    }
}
