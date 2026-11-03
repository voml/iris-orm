import { randomUUID } from "node:crypto";
import express from "express";

import type { PostId, UserId } from "@iris/index.ts";
import { createClient } from "@iris/node.ts";

const app = express();
app.use(express.json());

const db = createClient({ dsn: "file::memory:" });
await db.macros.seed_blog();

const postListSelect = {
    postId: true,
    title: true,
    published: true,
    author: { select: { userName: true } },
} as const;

app.get("/users", async (_req, res) => {
    res.json(await db.tables.user.findMany({ where: { active: true } }));
});

app.post("/users", async (req, res) => {
    const userName = typeof req.body?.user_name === "string" ? req.body.user_name.trim() : "";
    if (!userName) {
        res.status(400).json({ error: "user_name is required" });
        return;
    }
    const active = typeof req.body?.active === "boolean" ? req.body.active : true;
    const user = await db.tables.user.create({
        data: { userId: randomUUID() as UserId, userName, active },
    });
    res.status(201).json(user);
});

app.get("/posts", async (req, res) => {
    const author = typeof req.query.author === "string" ? req.query.author.trim() : "";
    const where = author
        ? { published: true, author: { userName: author } }
        : { published: true };
    res.json(await db.tables.post.findMany({ where, select: postListSelect }));
});

app.post("/posts", async (req, res) => {
    const authorUserId = typeof req.body?.author_user_id === "string" ? req.body.author_user_id.trim() : "";
    const title = typeof req.body?.title === "string" ? req.body.title.trim() : "";
    if (!authorUserId || !title) {
        res.status(400).json({ error: "author_user_id and title are required" });
        return;
    }
    const published = typeof req.body?.published === "boolean" ? req.body.published : true;
    const post = await db.tables.post.create({
        data: {
            postId: randomUUID() as PostId,
            author: authorUserId as UserId,
            title,
            published,
        },
    });
    res.status(201).json(post);
});

const port = Number(process.env.PORT ?? 3000);

const server = app.listen(port, () => {
    console.log(`@yydb/iris + Express → http://127.0.0.1:${port}`);
});

for (const signal of ["SIGINT", "SIGTERM"] as const) {
    process.on(signal, () => {
        server.close();
        process.exit(0);
    });
}
