import { loadIrisWeb } from "@yydb/iris/wasm";

import { Database, type DbClient } from "@iris/browser.ts";

import { IRIS_SCHEMA } from "./schema.ts";

let irisDatabase: DbClient | null = null;

/** WASM generated client (memory per isolate). Not D1-backed yet. */
export async function openIrisDatabase(): Promise<DbClient> {
    if (!irisDatabase) {
        await loadIrisWeb();
        irisDatabase = await Database.create({ schema: IRIS_SCHEMA });
        await irisDatabase.$macros.seed_blog();
    }
    return irisDatabase;
}
