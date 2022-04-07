import type { APIRoute } from "astro";

import { createUser, listUsers, openIrisDb } from "../../lib/iris.ts";

export const GET: APIRoute = async () => {
    const db = await openIrisDb();
    return new Response(JSON.stringify(await listUsers(db)), {
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
    const db = await openIrisDb();
    const user = await createUser(db, userName, body.active ?? true);
    return new Response(JSON.stringify(user), {
        status: 201,
        headers: { "content-type": "application/json" },
    });
};
