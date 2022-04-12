import { randomUUID } from "node:crypto";

import type { UserId } from "@iris/index.ts";
import { createDb } from "@iris/node.ts";

export const runtime = "nodejs";

export async function GET() {
    const db = await createDb({ config: process.cwd(), source: "default" });
    return Response.json(await db.user.findMany({ where: { active: true } }));
}

export async function POST(request: Request) {
    const body = (await request.json()) as { user_name?: string; active?: boolean };
    const userName = body.user_name?.trim() ?? "";
    if (!userName) {
        return Response.json({ error: "user_name is required" }, { status: 400 });
    }
    const db = await createDb({ config: process.cwd(), source: "default" });
    const user = await db.user.create({
        data: {
            user_id: randomUUID() as UserId,
            user_name: userName,
            active: body.active ?? true,
        },
    });
    return Response.json(user, { status: 201 });
}
