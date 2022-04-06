import { createPost, openIrisDb } from "../utils/iris.ts";

export default defineEventHandler(async (event) => {
    const body = await readBody<{ author_user_id?: string; title?: string; published?: boolean }>(event);
    const authorUserId = body.author_user_id?.trim() ?? "";
    const title = body.title?.trim() ?? "";
    if (!authorUserId || !title) {
        throw createError({ statusCode: 400, statusMessage: "author_user_id and title are required" });
    }
    const db = await openIrisDb();
    return await createPost(db, {
        author_user_id: authorUserId,
        title,
        published: body.published,
    });
});
