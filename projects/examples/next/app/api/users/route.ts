import { randomUUID } from "node:crypto";

import { getDb } from "../../../lib/db.ts";
import type { UserId } from "../../../src/generated/iris/references.ts";

export const runtime = "nodejs";

export async function GET() {
    const db = await getDb();
    return Response.json(await db.user.findMany({ where: { active: true } }));
}

export async function POST(request: Request) {
    const body = (await request.json()) as { user_name?: string; active?: boolean };
    const userName = body.user_name?.trim() ?? "";
    if (!userName) {
        return Response.json({ error: "user_name is required" }, { status: 400 });
    }
    const db = await getDb();
    const user = await db.user.create({
        data: {
            user_id: randomUUID() as UserId,
            user_name: userName,
            active: body.active ?? true,
        },
    });
    return Response.json(user, { status: 201 });
}
