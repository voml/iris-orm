import { randomUUID } from "node:crypto";
import { json, type RequestEvent } from "@sveltejs/kit";

import { openDatabase } from "@iris/node.ts";
import type { UserId } from "@iris/index.ts";

export async function GET() {
    const db = await openDatabase({ config: process.cwd(), source: "default" });
    return json(await db.user.findMany({ where: { active: true } }));
}

export async function POST({ request }: RequestEvent) {
    const body = (await request.json()) as { user_name?: string; active?: boolean };
    const userName = body.user_name?.trim() ?? "";
    if (!userName) {
        return json({ error: "user_name is required" }, { status: 400 });
    }
    const db = await openDatabase({ config: process.cwd(), source: "default" });
    const user = await db.user.create({
        data: {
            user_id: randomUUID() as UserId,
            user_name: userName,
            active: body.active ?? true,
        },
    });
    return json(user, { status: 201 });
}
