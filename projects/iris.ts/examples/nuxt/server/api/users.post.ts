import { randomUUID } from "node:crypto";

import type { UserId } from "@iris/index.ts";
import { Database } from "@iris/node.ts";

export default defineEventHandler(async (event) => {
    const body = await readBody<{ user_name?: string; active?: boolean }>(event);
    const userName = body.user_name?.trim() ?? "";
    if (!userName) {
        throw createError({ statusCode: 400, statusMessage: "user_name is required" });
    }
    const db = await Database.open({ config: process.cwd(), source: "default" });
    return await db.user.create({
        data: {
            userId: randomUUID() as UserId,
            userName: userName,
            active: body.active ?? true,
        },
    });
});
