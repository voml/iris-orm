import { Hono } from "hono";

import type { PostId, UserId } from "@iris/index.ts";

import { d1InsertPost, d1InsertUser, d1ListActiveUsers, d1ListPublishedPosts } from "./d1-blog.ts";
import { openIrisDatabase } from "./iris-session.ts";

const postListSelect = {
    post_id: true,
    title: true,
    published: true,
    author: { select: { user_name: true } },
} as const;

const app = new Hono<{ Bindings: Env }>();

app.get("/health", async (c) => {
    const d1 = await c.env.DB.prepare("SELECT 1 AS ok").first<{ ok: number }>();
    return c.json({
        ok: true,
        d1: d1?.ok === 1,
        iris: "wasm-memory",
        note: "D1 routes persist. /iris/* uses generated WASM client until Iris D1 session ships.",
    });
});

/** D1-backed API (target persistence path for Workers). */
app.get("/users", async (c) => {
    const rows = await d1ListActiveUsers(c.env.DB);
    return c.json(
        rows.map((row) => ({
            user_id: row.user_id,
            user_name: row.user_name,
            active: row.active === 1,
        })),
    );
});

app.post("/users", async (c) => {
    const body = await c.req.json<{ user_name?: string; active?: boolean }>();
    const userName = body.user_name?.trim() ?? "";
    if (!userName) {
        return c.json({ error: "user_name is required" }, 400);
    }
    const userId = crypto.randomUUID();
    await d1InsertUser(c.env.DB, {
        user_id: userId,
        user_name: userName,
        active: body.active ?? true,
    });
    return c.json({ user_id: userId, user_name: userName, active: body.active ?? true }, 201);
});

app.get("/posts", async (c) => {
    const author = c.req.query("author")?.trim();
    const rows = await d1ListPublishedPosts(c.env.DB, author);
    return c.json(
        rows.map((row) => ({
            post_id: row.post_id,
            title: row.title,
            published: row.published === 1,
            author: { user_name: row.author_name ?? "" },
        })),
    );
});

app.post("/posts", async (c) => {
    const body = await c.req.json<{ author_user_id?: string; title?: string; published?: boolean }>();
    const authorUserId = body.author_user_id?.trim() ?? "";
    const title = body.title?.trim() ?? "";
    if (!authorUserId || !title) {
        return c.json({ error: "author_user_id and title are required" }, 400);
    }
    const postId = crypto.randomUUID();
    await d1InsertPost(c.env.DB, {
        post_id: postId,
        author: authorUserId,
        title,
        published: body.published ?? true,
    });
    return c.json(
        {
            post_id: postId,
            author: authorUserId,
            title,
            published: body.published ?? true,
        },
        201,
    );
});

/** Generated Iris client on WASM (same routes as Hono example; isolate memory only). */
app.get("/iris/users", async (c) => {
    const db = await openIrisDatabase();
    return c.json(await db.user.findMany({ where: { active: true } }));
});

app.get("/iris/posts", async (c) => {
    const author = c.req.query("author")?.trim();
    const db = await openIrisDatabase();
    const where = author
        ? { published: true, author: { user_name: author } }
        : { published: true };
    return c.json(await db.post.findMany({ where, select: postListSelect }));
});

app.post("/iris/users", async (c) => {
    const body = await c.req.json<{ user_name?: string; active?: boolean }>();
    if (!body.user_name?.trim()) {
        return c.json({ error: "user_name is required" }, 400);
    }
    const db = await openIrisDatabase();
    const user = await db.user.create({
        data: {
            user_id: crypto.randomUUID() as UserId,
            user_name: body.user_name.trim(),
            active: body.active ?? true,
        },
    });
    return c.json(user, 201);
});

app.post("/iris/posts", async (c) => {
    const body = await c.req.json<{ author_user_id?: string; title?: string; published?: boolean }>();
    const authorUserId = body.author_user_id?.trim() ?? "";
    const title = body.title?.trim() ?? "";
    if (!authorUserId || !title) {
        return c.json({ error: "author_user_id and title are required" }, 400);
    }
    const db = await openIrisDatabase();
    const post = await db.post.create({
        data: {
            post_id: crypto.randomUUID() as PostId,
            author: authorUserId as UserId,
            title,
            published: body.published ?? true,
        },
    });
    return c.json(post, 201);
});

export default app;
