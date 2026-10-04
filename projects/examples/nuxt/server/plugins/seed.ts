import { Database } from "@iris/node.ts";

export default defineNitroPlugin(async () => {
    const db = await Database.open({ config: process.cwd(), source: "default" });
    await db.$macros.seed_blog();
});
