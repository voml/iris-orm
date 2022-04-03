import Fastify from "fastify";

import { closeIrisDb, createUser, listUsers, openIrisDb } from "./iris.ts";

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
