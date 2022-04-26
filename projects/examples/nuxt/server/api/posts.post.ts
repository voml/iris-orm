import { randomUUID } from "node:crypto";

import type { PostId, UserId } from "../../src/generated/iris/references.ts";
import { useDb } from "../utils/db.ts";

export default defineEventHandler(async (event) => {
    const body = await readBody<{ author_user_id?: string; title?: string; published?: boolean }>(event);
    const authorUserId = body.author_user_id?.trim() ?? "";
    const title = body.title?.trim() ?? "";
    if (!authorUserId || !title) {
        throw createError({ statusCode: 400, statusMessage: "author_user_id and title are required" });
    }
    const db = await useDb();
    return await db.post.create({
        data: {
            post_id: randomUUID() as PostId,
            author: authorUserId as UserId,
            title,
            published: body.published ?? true,
        },
    });
});
