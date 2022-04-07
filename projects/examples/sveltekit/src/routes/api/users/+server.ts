import { json } from "@sveltejs/kit";

import { createUser, listUsers, openIrisDb } from "$lib/iris.ts";
import type { RequestHandler } from "./$types";

export const GET: RequestHandler = async () => {
    const db = await openIrisDb();
    return json(await listUsers(db));
};

export const POST: RequestHandler = async ({ request }) => {
    const body = (await request.json()) as { user_name?: string; active?: boolean };
    const userName = body.user_name?.trim() ?? "";
    if (!userName) {
        return json({ error: "user_name is required" }, { status: 400 });
    }
    const db = await openIrisDb();
    const user = await createUser(db, userName, body.active ?? true);
    return json(user, { status: 201 });
};
