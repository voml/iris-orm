import { defineMiddleware } from "astro:middleware";

import { Database } from "@iris/node.ts";

let seeded = false;

export const onRequest = defineMiddleware(async (_context, next) => {
    if (!seeded) {
        const db = await Database.open({ config: process.cwd(), source: "default" });
        await db.$macros.seed_blog();
        seeded = true;
    }
    return next();
});
