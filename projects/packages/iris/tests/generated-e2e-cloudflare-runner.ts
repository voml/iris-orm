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
                            return {
                                results: [{ user_id: "u1", user_name: "Ada", active: 1 }] as T[],
                            };
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
});
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
        createdActive: created.active,
        filteredTrace: traces[1],
        limitedTrace: traces[2],
        filteredLimitedTrace: traces[3],
        uniqueTrace: traces[4],
        createTrace: traces[5],
    }),
);
