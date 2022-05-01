import { randomUUID } from "node:crypto";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import Fastify from "fastify";

import type { PostId, UserId } from "@iris/index.ts";
import { closeDatabase, openDatabase } from "@iris/node.ts";

const projectRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const app = Fastify({ logger: false });
const db = await openDatabase({ config: projectRoot, source: "default" });

const postListSelect = {
    post_id: true,
    title: true,
    published: true,
    author: { select: { user_name: true } },
} as const;

app.get("/users", async () => {
    return await db.user.findMany({ where: { active: true } });
});

app.post<{ Body: { user_name?: string; active?: boolean } }>("/users", async (request, reply) => {
    const userName = request.body.user_name?.trim() ?? "";
    if (!userName) {
        return reply.code(400).send({ error: "user_name is required" });
    }
    const user = await db.user.create({
        data: {
            user_id: randomUUID() as UserId,
            user_name: userName,
            active: request.body.active ?? true,
        },
    });
    return reply.code(201).send(user);
});

app.get<{ Querystring: { author?: string } }>("/posts", async (request) => {
    const author = request.query.author?.trim();
    const where = author
        ? { published: true, author: { user_name: author } }
        : { published: true };
    return await db.post.findMany({ where, select: postListSelect });
});

app.post<{ Body: { author_user_id?: string; title?: string; published?: boolean } }>("/posts", async (request, reply) => {
    const authorUserId = request.body.author_user_id?.trim() ?? "";
    const title = request.body.title?.trim() ?? "";
    if (!authorUserId || !title) {
        return reply.code(400).send({ error: "author_user_id and title are required" });
    }
    const post = await db.post.create({
        data: {
            post_id: randomUUID() as PostId,
            author: authorUserId as UserId,
            title,
            published: request.body.published ?? true,
        },
    });
    return reply.code(201).send(post);
});

const port = Number(process.env.PORT ?? 3001);

await app.listen({ port, host: "127.0.0.1" });
console.log(`@yydb/iris + Fastify → http://127.0.0.1:${port}`);

for (const signal of ["SIGINT", "SIGTERM"] as const) {
    process.on(signal, async () => {
        await app.close();
        await closeDatabase();
        process.exit(0);
    });
}
