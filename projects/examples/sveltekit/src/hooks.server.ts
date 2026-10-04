import type { Handle } from "@sveltejs/kit";

import { Database } from "@iris/node.ts";

let seeded = false;

export const handle: Handle = async ({ event, resolve }) => {
    if (!seeded) {
        const db = await Database.open({ config: process.cwd(), source: "default" });
        await db.$macros.seed_blog();
        seeded = true;
    }
    return resolve(event);
};
