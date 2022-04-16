import { randomUUID } from "node:crypto";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import express from "express";

import type { PostId, UserId } from "@iris/index.ts";
import { closeDatabase, openDatabase } from "@iris/node.ts";

const projectRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const app = express();
app.use(express.json());

const db = await openDatabase({ config: projectRoot, source: "default" });

const postListSelect = {
    post_id: true,
    title: true,
    published: true,
    author: { select: { user_name: true } },
} as const;

app.get("/users", async (_req, res) => {
    res.json(await db.user.findMany({ where: { active: true } }));
});

app.post("/users", async (req, res) => {
    const userName = typeof req.body?.user_name === "string" ? req.body.user_name.trim() : "";
    if (!userName) {
        res.status(400).json({ error: "user_name is required" });
        return;
    }
    const active = typeof req.body?.active === "boolean" ? req.body.active : true;
    const user = await db.user.create({
        data: { user_id: randomUUID() as UserId, user_name: userName, active },
    });
    res.status(201).json(user);
});

app.get("/posts", async (req, res) => {
    const author = typeof req.query.author === "string" ? req.query.author.trim() : "";
    const where = author
        ? { published: true, author: { user_name: author } }
        : { published: true };
    res.json(await db.post.findMany({ where, select: postListSelect }));
});

app.post("/posts", async (req, res) => {
    const authorUserId = typeof req.body?.author_user_id === "string" ? req.body.author_user_id.trim() : "";
    const title = typeof req.body?.title === "string" ? req.body.title.trim() : "";
    if (!authorUserId || !title) {
        res.status(400).json({ error: "author_user_id and title are required" });
        return;
    }
    const published = typeof req.body?.published === "boolean" ? req.body.published : true;
    const post = await db.post.create({
        data: {
            post_id: randomUUID() as PostId,
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
    process.on(signal, async () => {
        server.close();
        await closeDatabase();
        process.exit(0);
    });
}
