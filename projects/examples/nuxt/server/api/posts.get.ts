import { openDatabase } from "@iris/node.ts";

const postListSelect = {
    post_id: true,
    title: true,
    published: true,
    author: { select: { user_name: true } },
} as const;

export default defineEventHandler(async (event) => {
    const author = getQuery(event).author;
    const db = await openDatabase({ config: process.cwd(), source: "default" });
    const authorName = typeof author === "string" ? author.trim() : "";
    const where = authorName
        ? { published: true, author: { user_name: authorName } }
        : { published: true };
    return await db.post.findMany({ where, select: postListSelect });
});
