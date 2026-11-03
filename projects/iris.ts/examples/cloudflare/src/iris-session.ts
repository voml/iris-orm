import { loadIrisWeb } from "@yydb/iris-language";

import { createClient, type Client } from "@iris/browser.ts";

let irisDatabase: Client | null = null;

/** WASM generated client (memory per isolate). Not D1-backed yet. */
export async function openIrisDatabase(): Promise<Client> {
    if (!irisDatabase) {
        await loadIrisWeb();
        irisDatabase = createClient({ dsn: "memory:" });
        await irisDatabase.macros.seed_blog();
    }
    return irisDatabase;
}
