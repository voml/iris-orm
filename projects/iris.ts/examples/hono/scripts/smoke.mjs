import { randomUUID } from "node:crypto";

import { userId } from "../src/generated/iris/references.ts";
import { createClient } from "../src/generated/iris/node.ts";

const db = createClient({ dsn: "file::memory:" });

await db.macros.seed_blog();
const users = await db.tables.user.findMany({ where: { active: true } });
if (users.length !== 2) {
    throw new Error(`expected 2 seeded users, got ${users.length}`);
}

const posts = await db.tables.post.findMany({ where: { published: true } });
if (posts.length !== 2) {
    throw new Error(`expected 2 seeded posts, got ${posts.length}`);
}

const adaPosts = await db.tables.post.findMany({
    where: { published: true, author: { userName: "ada" } },
});
if (adaPosts.length !== 1 || adaPosts[0]?.title !== "Ada post") {
    throw new Error(`unexpected ada posts: ${JSON.stringify(adaPosts)}`);
}

const created = await db.tables.user.create({
    data: {
        userId: userId(randomUUID()),
        userName: "smoke",
        active: true,
    },
});
const createdName = created?.userName;
if (createdName !== "smoke") {
    throw new Error(`unexpected created user: ${JSON.stringify(created)}`);
}

console.log("hono smoke ok");
