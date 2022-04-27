import { randomUUID } from "node:crypto";
import { json, type RequestEvent } from "@sveltejs/kit";

import { getDb } from "$lib/server/db.ts";
import type { UserId } from "../../../generated/iris/references.ts";

export async function GET() {
    const db = await getDb();
    return json(await db.user.findMany({ where: { active: true } }));
}

export async function POST({ request }: RequestEvent) {
    const body = (await request.json()) as { user_name?: string; active?: boolean };
    const userName = body.user_name?.trim() ?? "";
    if (!userName) {
        return json({ error: "user_name is required" }, { status: 400 });
    }
    const db = await getDb();
    const user = await db.user.create({
        data: {
            user_id: randomUUID() as UserId,
            user_name: userName,
            active: body.active ?? true,
        },
    });
    return json(user, { status: 201 });
}
