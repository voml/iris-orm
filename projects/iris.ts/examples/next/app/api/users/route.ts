import { randomUUID } from "node:crypto";

import type { UserId } from "@iris/index.ts";
import { createClient } from "@iris/node.ts";

export const runtime = "nodejs";

export async function GET() {
    const db = createClient({ dsn: "file:.iris/dev.yydb" });
    return Response.json(await db.tables.user.findMany({ where: { active: true } }));
}

export async function POST(request: Request) {
    const body = (await request.json()) as { user_name?: string; active?: boolean };
    const userName = body.user_name?.trim() ?? "";
    if (!userName) {
        return Response.json({ error: "user_name is required" }, { status: 400 });
    }
    const db = createClient({ dsn: "file:.iris/dev.yydb" });
    const user = await db.tables.user.create({
        data: {
            userId: randomUUID() as UserId,
            userName: userName,
            active: body.active ?? true,
        },
    });
    return Response.json(user, { status: 201 });
}
