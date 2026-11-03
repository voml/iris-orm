import { createClient } from "@iris/node.ts";

export default defineEventHandler(async () => {
    const db = createClient({ dsn: "file:.iris/dev.yydb" });
    return await db.tables.user.findMany({ where: { active: true } });
});
