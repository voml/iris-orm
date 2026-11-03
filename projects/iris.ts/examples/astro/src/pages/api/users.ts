import { randomUUID } from "node:crypto";
import type { APIRoute } from "astro";

import { createClient } from "@iris/node.ts";
import type { UserId } from "@iris/index.ts";

export const GET: APIRoute = async () => {
    const db = createClient({ dsn: "file:.iris/dev.yydb" });
    return new Response(JSON.stringify(await db.tables.user.findMany({ where: { active: true } })), {
        headers: { "content-type": "application/json" },
    });
};

export const POST: APIRoute = async ({ request }) => {
    const body = (await request.json()) as { user_name?: string; active?: boolean };
    const userName = body.user_name?.trim() ?? "";
    if (!userName) {
        return new Response(JSON.stringify({ error: "user_name is required" }), {
            status: 400,
            headers: { "content-type": "application/json" },
        });
    }
    const db = createClient({ dsn: "file:.iris/dev.yydb" });
    const user = await db.tables.user.create({
        data: {
            userId: randomUUID() as UserId,
            userName: userName,
            active: body.active ?? true,
        },
    });
    return new Response(JSON.stringify(user), {
        status: 201,
        headers: { "content-type": "application/json" },
    });
};
