import { useDb } from "../utils/db.ts";

const postListSelect = {
    post_id: true,
    title: true,
    published: true,
    author: { user_name: true },
} as const;

export default defineEventHandler(async (event) => {
    const author = getQuery(event).author;
    const db = await useDb();
    const authorName = typeof author === "string" ? author.trim() : "";
    const where = authorName
        ? { published: true, author: { user_name: authorName } }
        : { published: true };
    return await db.post.findMany({ where, select: postListSelect });
});
