import { Database } from "@iris/node.ts";

export default defineEventHandler(async () => {
    const db = await Database.open({ config: process.cwd(), source: "default" });
    return await db.user.findMany({ where: { active: true } });
});
