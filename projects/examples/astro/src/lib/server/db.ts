import { createDb, type DbClient } from "@iris/node.ts";

let db: DbClient | null = null;

/** Server-only singleton (`output: "server"` + `@astrojs/node`). */
export async function getDb(): Promise<DbClient> {
    if (!db) {
        db = await createDb({ config: process.cwd(), source: "default" });
    }
    return db;
}
