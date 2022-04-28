import { randomUUID } from "node:crypto";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { closeDatabase, openDatabase } from "../src/generated/iris/node.ts";

const projectRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const db = await openDatabase({ config: projectRoot, source: "default" });

await db.$macros.seed_blog();
const users = await db.user.findMany({ where: { active: true } });
if (users.length !== 2) {
    throw new Error(`expected 2 seeded users, got ${users.length}`);
}

const created = await db.user.create({
    data: {
        user_id: randomUUID(),
        user_name: "smoke",
        active: true,
    },
});
if (created?.user_name !== "smoke") {
    throw new Error(`unexpected created user: ${JSON.stringify(created)}`);
}

await closeDatabase();
console.log("hono smoke ok");
