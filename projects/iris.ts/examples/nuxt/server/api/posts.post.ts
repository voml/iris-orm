import { randomUUID } from "node:crypto";

import type { PostId, UserId } from "@iris/index.ts";
import { createClient } from "@iris/node.ts";

export default defineEventHandler(async (event) => {
    const body = await readBody<{ author_user_id?: string; title?: string; published?: boolean }>(event);
    const authorUserId = body.author_user_id?.trim() ?? "";
    const title = body.title?.trim() ?? "";
    if (!authorUserId || !title) {
        throw createError({ statusCode: 400, statusMessage: "author_user_id and title are required" });
    }
    const db = createClient({ dsn: "file:.iris/dev.yydb" });
    return await db.tables.post.create({
        data: {
            postId: randomUUID() as PostId,
            author: authorUserId as UserId,
            title,
            published: body.published ?? true,
        },
    });
});
