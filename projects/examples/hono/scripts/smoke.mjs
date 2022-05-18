import { randomUUID } from "node:crypto";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { userId } from "../src/generated/iris/references.ts";
import { closeDatabase, openDatabase } from "../src/generated/iris/node.ts";

const projectRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const db = await openDatabase({ config: projectRoot, source: "default" });

await db.$macros.seed_blog();
const users = await db.user.findMany({ where: { active: true } });
if (users.length !== 2) {
    throw new Error(`expected 2 seeded users, got ${users.length}`);
}

const posts = await db.post.findMany({ where: { published: true } });
if (posts.length !== 2) {
    throw new Error(`expected 2 seeded posts, got ${posts.length}`);
}

const adaPosts = await db.post.findMany({
    where: { published: true, author: { userName: "ada" } },
});
if (adaPosts.length !== 1 || adaPosts[0]?.title !== "Ada post") {
    throw new Error(`unexpected ada posts: ${JSON.stringify(adaPosts)}`);
}

const created = await db.user.create({
    data: {
        userId: userId(randomUUID()),
        userName: "smoke",
        active: true,
    },
});
const createdName = created?.userName ?? created?.user_name;
if (createdName !== "smoke") {
    throw new Error(`unexpected created user: ${JSON.stringify(created)}`);
}

await closeDatabase();
console.log("hono smoke ok");
