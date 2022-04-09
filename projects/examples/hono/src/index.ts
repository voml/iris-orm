import { serve } from "@hono/node-server";
import { Hono } from "hono";

import { closeIrisDb, createPost, createUser, listPosts, listPostsByAuthorName, listUsers, openIrisDb } from "./iris.ts";

const app = new Hono();
const db = await openIrisDb();

app.get("/users", async (c) => {
    return c.json(await listUsers(db));
});

app.post("/users", async (c) => {
    const body = await c.req.json<{ user_name?: string; active?: boolean }>();
    if (!body.user_name?.trim()) {
        return c.json({ error: "user_name is required" }, 400);
    }
    const user = await createUser(db, body.user_name.trim(), body.active ?? true);
    return c.json(user, 201);
});

app.get("/posts", async (c) => {
    const author = c.req.query("author")?.trim();
    if (author) {
        return c.json(await listPostsByAuthorName(db, author));
    }
    return c.json(await listPosts(db));
});

app.post("/posts", async (c) => {
    const body = await c.req.json<{ author_user_id?: string; title?: string; published?: boolean }>();
    const authorUserId = body.author_user_id?.trim() ?? "";
    const title = body.title?.trim() ?? "";
    if (!authorUserId || !title) {
        return c.json({ error: "author_user_id and title are required" }, 400);
    }
    const post = await createPost(db, {
        author_user_id: authorUserId,
        title,
        published: body.published,
    });
    return c.json(post, 201);
});

const port = Number(process.env.PORT ?? 8787);

serve({ fetch: app.fetch, port }, (info) => {
    console.log(`@yydb/iris + Hono → http://127.0.0.1:${info.port}`);
});

for (const signal of ["SIGINT", "SIGTERM"] as const) {
    process.on(signal, async () => {
        await closeIrisDb();
        process.exit(0);
    });
}
