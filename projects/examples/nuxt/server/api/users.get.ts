import { useDb } from "../utils/db.ts";

export default defineEventHandler(async () => {
    const db = await useDb();
    return await db.user.findMany({ where: { active: true } });
});
