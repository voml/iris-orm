import { createClient } from "@iris/node.ts";

export default defineNitroPlugin(async () => {
    const db = createClient({ dsn: "file:.iris/dev.yydb" });
    await db.macros.seed_blog();
});
