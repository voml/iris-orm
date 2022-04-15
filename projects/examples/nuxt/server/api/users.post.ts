import { createUser, openIrisDb } from "../utils/iris.ts";

export default defineEventHandler(async (event) => {
    const body = await readBody<{ user_name?: string; active?: boolean }>(event);
    const userName = body.user_name?.trim() ?? "";
    if (!userName) {
        throw createError({ statusCode: 400, statusMessage: "user_name is required" });
    }
    const db = await openIrisDb();
    return await createUser(db, userName, body.active ?? true);
});
