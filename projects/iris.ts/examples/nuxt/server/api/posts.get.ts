import { Database } from "@iris/node.ts";

const postListSelect = {
    postId: true,
    title: true,
    published: true,
    author: { select: { userName: true } },
} as const;

export default defineEventHandler(async (event) => {
    const author = getQuery(event).author;
    const db = await Database.open({ config: process.cwd(), source: "default" });
    const authorName = typeof author === "string" ? author.trim() : "";
    const where = authorName
        ? { published: true, author: { userName: authorName } }
        : { published: true };
    return await db.post.findMany({ where, select: postListSelect });
});
