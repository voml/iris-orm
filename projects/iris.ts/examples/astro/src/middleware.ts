import { defineMiddleware } from "astro:middleware";

import { createClient } from "@iris/node.ts";

let seeded = false;

export const onRequest = defineMiddleware(async (_context, next) => {
    if (!seeded) {
        const db = createClient({ dsn: "file:.iris/dev.yydb" });
        await db.macros.seed_blog();
        seeded = true;
    }
    return next();
});
