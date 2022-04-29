import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { createDb, type DbClient } from "@iris/node.ts";

const projectRoot = join(dirname(fileURLToPath(import.meta.url)), "..");

let db: DbClient | null = null;

/** Process-wide generated client (`iris generate --config .`). */
export async function getDb(): Promise<DbClient> {
    if (!db) {
        db = await createDb({ config: projectRoot, source: "default" });
    }
    return db;
}

export async function closeDb(): Promise<void> {
    if (!db) {
        return;
    }
    await db.$close();
    db = null;
}
