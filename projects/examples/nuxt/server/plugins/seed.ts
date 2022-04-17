import { openDatabase } from "@iris/node.ts";

export default defineNitroPlugin(async () => {
    const db = await openDatabase({ config: process.cwd(), source: "default" });
    await db.$macros.seed_blog();
});
