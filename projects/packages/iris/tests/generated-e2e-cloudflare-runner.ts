import { Database } from "./cloudflare.js";

type Trace = { sql: string; bind: unknown[] };

function createFakeD1() {
    const traces: Trace[] = [];
    const d1 = {
        prepare(query: string) {
            return {
                bind(...values: unknown[]) {
                    traces.push({ sql: query, bind: values });
                    return {
                        async all<T>() {
                            const full = { user_id: "u1", user_name: "Ada", active: 1 };
                            const returningMatch = query.match(/RETURNING\s+(.+)$/i);
                            if (!returningMatch) {
                                return { results: [full] as T[] };
                            }
                            const row: Record<string, unknown> = {};
                            for (const col of returningMatch[1].split(",").map((name) => name.trim())) {
                                if (col in full) {
                                    row[col] = full[col as keyof typeof full];
                                }
                            }
                            return { results: [row] as T[] };
                        },
                        async run() {
                            return { success: true };
                        },
                    };
                },
            };
        },
    };
    return { d1, traces };
}

const { d1, traces } = createFakeD1();
const db = await Database.create({ d1 });
const unfiltered = await db.user.findMany();
const filtered = await db.user.findMany({ where: { active: { eq: true } } });
const limited = await db.user.findMany({ take: 2 });
const filteredLimited = await db.user.findMany({ where: { active: { eq: true } }, take: 2 });
const unique = await db.user.findUnique({ where: { userId: "u1" } });
const created = await db.user.create({
    data: { userId: "u2", userName: "Bob", active: false },
    select: { userId: true, userName: true },
});
const updated = await db.user.update({
    where: { userId: "u2" },
    data: { userName: { set: "Carol" } },
});
await db.user.delete({ where: { userId: "u2" } });
await db.$close();

console.log(
    JSON.stringify({
        ok: true,
        unfilteredCount: unfiltered.length,
        filteredCount: filtered.length,
        limitedCount: limited.length,
        filteredLimitedCount: filteredLimited.length,
        uniqueUserId: unique?.userId,
        createdUserId: created.userId,
        createdUserName: created.userName,
        createdHasActive: "active" in created,
        updatedUserName: updated.userName,
        filteredTrace: traces[1],
        limitedTrace: traces[2],
        filteredLimitedTrace: traces[3],
        uniqueTrace: traces[4],
        createTrace: traces[5],
        updateTrace: traces[6],
        deleteTrace: traces[7],
    }),
);
