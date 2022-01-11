/**
 * Isolated MySQL foreign-store adapter (`@yydb/iris-adapter-mysql`).
 *
 * Driver choice: **mysql2** in JS mode (do not enable native addon builds).
 * SQL stays private to this package.
 */

import type { IrisPlaceholder } from "@yydb/iris-types";

/** Pairs with Rust `iris-adapter-mysql`. */
export const BACKEND_ID = "mysql" as const;

export type MysqlAdapterOptions = {
    url: string;
};

/** Placeholder source handle until catalog / execute / migrate land. */
export class MysqlSource {
    readonly backendId = BACKEND_ID;

    constructor(readonly options: MysqlAdapterOptions) {}

    placeholder(): IrisPlaceholder {
        return { __irisTypes: "native-ts-skeleton" };
    }
}
