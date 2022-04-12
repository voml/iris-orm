import { createDb, type DbClient } from "@iris/node.ts";

let db: DbClient | null = null;

/** Nitro server singleton over the generated client (`iris generate --config .`). */
export async function useDb(): Promise<DbClient> {
    if (!db) {
        db = await createDb({ config: process.cwd(), source: "default" });
    }
    return db;
}
