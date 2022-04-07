import { listUsers, openIrisDb } from "../utils/iris.ts";

export default defineEventHandler(async () => {
    const db = await openIrisDb();
    return await listUsers(db);
});
