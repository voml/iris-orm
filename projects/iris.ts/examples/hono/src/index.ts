import { randomUUID } from "node:crypto";
import { serve } from "@hono/node-server";
import { Hono } from "hono";

import type { PostId, UserId } from "@iris/index.ts";
import { createClient } from "@iris/node.ts";

const app = new Hono();
const db = createClient({ dsn: "file::memory:" });
await db.macros.seed_blog();

const postListSelect = {
    postId: true,
    title: true,
    published: true,
    author: { select: { userName: true } },
} as const;

app.get("/users", async (c) => {
    return c.json(await db.tables.user.findMany({ where: { active: true } }));
});

app.post("/users", async (c) => {
    const body = await c.req.json<{ user_name?: string; active?: boolean }>();
    if (!body.user_name?.trim()) {
        return c.json({ error: "user_name is required" }, 400);
    }
    const user = await db.tables.user.create({
        data: {
            userId: randomUUID() as UserId,
            userName: body.user_name.trim(),
            active: body.active ?? true,
        },
    });
    return c.json(user, 201);
});

app.get("/posts", async (c) => {
    const author = c.req.query("author")?.trim();
    const where = author
        ? { published: true, author: { userName: author } }
        : { published: true };
    return c.json(await db.tables.post.findMany({ where, select: postListSelect }));
});

app.post("/posts", async (c) => {
    const body = await c.req.json<{ author_user_id?: string; title?: string; published?: boolean }>();
    const authorUserId = body.author_user_id?.trim() ?? "";
    const title = body.title?.trim() ?? "";
    if (!authorUserId || !title) {
        return c.json({ error: "author_user_id and title are required" }, 400);
    }
    const post = await db.tables.post.create({
        data: {
            postId: randomUUID() as PostId,
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
