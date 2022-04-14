import { randomUUID } from "node:crypto";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { createDb, type DbClient } from "../../../generated/iris/typescript/node.ts";

export type { Post, User } from "../../../generated/iris/typescript/models.ts";

const projectRoot = join(dirname(fileURLToPath(import.meta.url)), "../../..");

let db: DbClient | null = null;

/** Open the generated Iris client (`iris generate --config .`). */
export async function openIrisDb(): Promise<DbClient> {
    if (db) {
        return db;
    }
    db = await createDb({ config: projectRoot, source: "default" });
    await seedBlog(db);
    return db;
}

export async function closeIrisDb(): Promise<void> {
    if (!db) {
        return;
    }
    await db.$close();
    db = null;
}

export async function listUsers(db: DbClient) {
    return db.user.findMany({ where: { active: true } });
}

export async function createUser(db: DbClient, userName: string, active = true) {
    return db.user.create({
        data: { user_id: randomUUID(), user_name: userName, active },
    });
}

export async function listPosts(db: DbClient) {
    return db.post.findMany({
        where: { published: true },
        select: {
            post_id: true,
            title: true,
            published: true,
            author: { user_name: true },
        },
    });
}

/** Filter posts through generated `&User` where input (`author.user_name`). */
export async function listPostsByAuthorName(db: DbClient, authorName: string) {
    return db.post.findMany({
        where: { author: { user_name: authorName } },
        select: {
            post_id: true,
            title: true,
            published: true,
            author: { user_name: true },
        },
    });
}

export async function createPost(
    db: DbClient,
    input: { author_user_id: string; title: string; published?: boolean },
) {
    return db.post.create({
        data: {
            post_id: randomUUID(),
            author: input.author_user_id,
            title: input.title,
            published: input.published ?? true,
        },
    });
}

async function seedBlog(db: DbClient): Promise<void> {
    if ((await listUsers(db)).length > 0) {
        return;
    }
    const ada = await createUser(db, "ada", true);
    const linus = await createUser(db, "linus", true);
    await createPost(db, {
        author_user_id: ada.user_id,
        title: "Hello &User refs",
        published: true,
    });
    await createPost(db, {
        author_user_id: linus.user_id,
        title: "Filter via x.author.user_name",
        published: true,
    });
}
