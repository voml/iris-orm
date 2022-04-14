import express from "express";

import { closeIrisDb, createUser, listUsers, openIrisDb } from "./iris.ts";

const app = express();
app.use(express.json());

const db = await openIrisDb();

app.get("/users", async (_req, res) => {
    res.json(await listUsers(db));
});

app.post("/users", async (req, res) => {
    const userName = typeof req.body?.user_name === "string" ? req.body.user_name.trim() : "";
    if (!userName) {
        res.status(400).json({ error: "user_name is required" });
        return;
    }
    const active = typeof req.body?.active === "boolean" ? req.body.active : true;
    const user = await createUser(db, userName, active);
    res.status(201).json(user);
});

const port = Number(process.env.PORT ?? 3000);

const server = app.listen(port, () => {
    console.log(`@yydb/iris + Express → http://127.0.0.1:${port}`);
});

for (const signal of ["SIGINT", "SIGTERM"] as const) {
    process.on(signal, async () => {
        server.close();
        await closeIrisDb();
        process.exit(0);
    });
}
