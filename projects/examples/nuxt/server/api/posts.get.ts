import { listPosts, listPostsByAuthorName, openIrisDb } from "../utils/iris.ts";

export default defineEventHandler(async (event) => {
    const author = getQuery(event).author;
    const db = await openIrisDb();
    if (typeof author === "string" && author.trim()) {
        return await listPostsByAuthorName(db, author.trim());
    }
    return await listPosts(db);
});
