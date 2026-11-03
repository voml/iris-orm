import { randomUUID } from "node:crypto";
import { json, type RequestEvent } from "@sveltejs/kit";

import { createClient } from "@iris/node.ts";
import type { UserId } from "@iris/index.ts";

export async function GET() {
    const db = createClient({ dsn: "file:.iris/dev.yydb" });
    return json(await db.tables.user.findMany({ where: { active: true } }));
}

export async function POST({ request }: RequestEvent) {
    const body = (await request.json()) as { user_name?: string; active?: boolean };
    const userName = body.user_name?.trim() ?? "";
    if (!userName) {
        return json({ error: "user_name is required" }, { status: 400 });
    }
    const db = createClient({ dsn: "file:.iris/dev.yydb" });
    const user = await db.tables.user.create({
        data: {
            userId: randomUUID() as UserId,
            userName: userName,
            active: body.active ?? true,
        },
    });
    return json(user, { status: 201 });
}
