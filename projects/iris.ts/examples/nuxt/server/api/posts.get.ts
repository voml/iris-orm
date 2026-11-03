import { createClient } from "@iris/node.ts";

const postListSelect = {
    postId: true,
    title: true,
    published: true,
    author: { select: { userName: true } },
} as const;

export default defineEventHandler(async (event) => {
    const author = getQuery(event).author;
    const db = createClient({ dsn: "file:.iris/dev.yydb" });
    const authorName = typeof author === "string" ? author.trim() : "";
    const where = authorName
        ? { published: true, author: { userName: authorName } }
        : { published: true };
    return await db.tables.post.findMany({ where, select: postListSelect });
});
