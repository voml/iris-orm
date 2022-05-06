import { defineMiddleware } from "astro:middleware";

import { openDatabase } from "@iris/node.ts";

let seeded = false;

export const onRequest = defineMiddleware(async (_context, next) => {
    if (!seeded) {
        const db = await openDatabase({ config: process.cwd(), source: "default" });
        await db.$macros.seed_blog();
        seeded = true;
    }
    return next();
});
