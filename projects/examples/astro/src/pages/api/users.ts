import { randomUUID } from "node:crypto";
import type { APIRoute } from "astro";

import { Database } from "@iris/node.ts";
import type { UserId } from "@iris/index.ts";

export const GET: APIRoute = async () => {
    const db = await Database.open({ config: process.cwd(), source: "default" });
    return new Response(JSON.stringify(await db.user.findMany({ where: { active: true } })), {
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
    const db = await Database.open({ config: process.cwd(), source: "default" });
    const user = await db.user.create({
        data: {
            user_id: randomUUID() as UserId,
            user_name: userName,
            active: body.active ?? true,
        },
    });
    return new Response(JSON.stringify(user), {
        status: 201,
        headers: { "content-type": "application/json" },
    });
};
