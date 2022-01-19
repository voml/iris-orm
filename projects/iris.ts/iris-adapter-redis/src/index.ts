/**
 * Isolated Redis foreign-store adapter (`@yydb/iris-adapter-redis`).
 *
 * Driver choice: **redis** (`node-redis`) — JS client, keyspace-only role.
 * Redis commands stay private to this package.
 */

import type { IrisPlaceholder } from "@yydb/iris-types";

/** Pairs with Rust `iris-adapter-redis`. */
export const BACKEND_ID = "redis" as const;

export type RedisAdapterOptions = {
    url: string;
};

/** Placeholder source handle until cache / mapping ops land. */
export class RedisSource {
    readonly backendId = BACKEND_ID;

    constructor(readonly options: RedisAdapterOptions) {}

    placeholder(): IrisPlaceholder {
        return { __irisTypes: "rust-binding-skeleton" };
    }
}
