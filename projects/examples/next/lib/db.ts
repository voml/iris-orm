import { randomUUID } from "node:crypto";
import { cache } from "react";

import { createDb, type DbClient } from "../src/generated/iris/node.ts";
import type { PostId, UserId } from "../src/generated/iris/references.ts";

let db: DbClient | null = null;

async function openDb(): Promise<DbClient> {
    if (!db) {
        db = await createDb({ config: process.cwd(), source: "default" });
        await seedBlog(db);
    }
    return db;
}

/** Request-deduped accessor; underlying client is a dev-server singleton. */
export const getDb = cache(openDb);

async function seedBlog(db: DbClient): Promise<void> {
    const existing = await db.user.findMany({ where: { active: true }, take: 1 });
    if (existing.length > 0) {
        return;
    }
    const adaId = randomUUID() as UserId;
    const linusId = randomUUID() as UserId;
    await db.user.create({
        data: { user_id: adaId, user_name: "ada", active: true },
    });
    await db.user.create({
        data: { user_id: linusId, user_name: "linus", active: true },
    });
    await db.post.create({
        data: {
            post_id: randomUUID() as PostId,
            author: adaId,
            title: "Hello &User refs",
            published: true,
        },
    });
    await db.post.create({
        data: {
            post_id: randomUUID() as PostId,
            author: linusId,
            title: "Filter via x.author.user_name",
            published: true,
        },
    });
}
