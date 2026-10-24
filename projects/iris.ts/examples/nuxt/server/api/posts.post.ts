import { randomUUID } from "node:crypto";

import type { PostId, UserId } from "@iris/index.ts";
import { Database } from "@iris/node.ts";

export default defineEventHandler(async (event) => {
    const body = await readBody<{ author_user_id?: string; title?: string; published?: boolean }>(event);
    const authorUserId = body.author_user_id?.trim() ?? "";
    const title = body.title?.trim() ?? "";
    if (!authorUserId || !title) {
        throw createError({ statusCode: 400, statusMessage: "author_user_id and title are required" });
    }
    const db = await Database.open({ config: process.cwd(), source: "default" });
    return await db.post.create({
        data: {
            post_id: randomUUID() as PostId,
            author: authorUserId as UserId,
            title,
            published: body.published ?? true,
        },
    });
});
