import { randomUUID } from "node:crypto";
import { serve } from "@hono/node-server";
import { Hono } from "hono";

import { closeDb, getDb } from "./db.ts";
import type { PostId, UserId } from "@iris/index.ts";

const app = new Hono();
const db = await getDb();

const postListSelect = {
    post_id: true,
    title: true,
    published: true,
    author: { user_name: true },
} as const;

app.get("/users", async (c) => {
    return c.json(await db.user.findMany({ where: { active: true } }));
});

app.post("/users", async (c) => {
    const body = await c.req.json<{ user_name?: string; active?: boolean }>();
    if (!body.user_name?.trim()) {
        return c.json({ error: "user_name is required" }, 400);
    }
    const user = await db.user.create({
        data: {
            user_id: randomUUID() as UserId,
            user_name: body.user_name.trim(),
            active: body.active ?? true,
        },
    });
    return c.json(user, 201);
});

app.get("/posts", async (c) => {
    const author = c.req.query("author")?.trim();
    const where = author
        ? { published: true, author: { user_name: author } }
        : { published: true };
    return c.json(await db.post.findMany({ where, select: postListSelect }));
});

app.post("/posts", async (c) => {
    const body = await c.req.json<{ author_user_id?: string; title?: string; published?: boolean }>();
    const authorUserId = body.author_user_id?.trim() ?? "";
    const title = body.title?.trim() ?? "";
    if (!authorUserId || !title) {
        return c.json({ error: "author_user_id and title are required" }, 400);
    }
    const post = await db.post.create({
        data: {
            post_id: randomUUID() as PostId,
            author: authorUserId as UserId,
            title,
            published: body.published ?? true,
        },
    });
    return c.json(post, 201);
});

const port = Number(process.env.PORT ?? 8787);

serve({ fetch: app.fetch, port }, (info) => {
    console.log(`@yydb/iris + Hono → http://127.0.0.1:${info.port}`);
});

for (const signal of ["SIGINT", "SIGTERM"] as const) {
    process.on(signal, async () => {
        await closeDb();
        process.exit(0);
    });
}
