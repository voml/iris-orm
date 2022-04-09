import Fastify from "fastify";

import { closeIrisDb, createPost, createUser, listPosts, listPostsByAuthorName, listUsers, openIrisDb } from "./iris.ts";

const app = Fastify({ logger: false });
const db = await openIrisDb();

app.get("/users", async () => {
    return await listUsers(db);
});

app.post<{ Body: { user_name?: string; active?: boolean } }>("/users", async (request, reply) => {
    const userName = request.body.user_name?.trim() ?? "";
    if (!userName) {
        return reply.code(400).send({ error: "user_name is required" });
    }
    const user = await createUser(db, userName, request.body.active ?? true);
    return reply.code(201).send(user);
});

app.get<{ Querystring: { author?: string } }>("/posts", async (request) => {
    const author = request.query.author?.trim();
    if (author) {
        return await listPostsByAuthorName(db, author);
    }
    return await listPosts(db);
});

app.post<{ Body: { author_user_id?: string; title?: string; published?: boolean } }>("/posts", async (request, reply) => {
    const authorUserId = request.body.author_user_id?.trim() ?? "";
    const title = request.body.title?.trim() ?? "";
    if (!authorUserId || !title) {
        return reply.code(400).send({ error: "author_user_id and title are required" });
    }
    const post = await createPost(db, {
        author_user_id: authorUserId,
        title,
        published: request.body.published,
    });
    return reply.code(201).send(post);
});

const port = Number(process.env.PORT ?? 3001);

await app.listen({ port, host: "127.0.0.1" });
console.log(`@yydb/iris + Fastify → http://127.0.0.1:${port}`);

for (const signal of ["SIGINT", "SIGTERM"] as const) {
    process.on(signal, async () => {
        await app.close();
        await closeIrisDb();
        process.exit(0);
    });
}
