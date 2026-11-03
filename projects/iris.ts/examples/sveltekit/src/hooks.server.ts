import type { Handle } from "@sveltejs/kit";

import { createClient } from "@iris/node.ts";

let seeded = false;

export const handle: Handle = async ({ event, resolve }) => {
    if (!seeded) {
        const db = createClient({ dsn: "file:.iris/dev.yydb" });
        await db.macros.seed_blog();
        seeded = true;
    }
    return resolve(event);
};
