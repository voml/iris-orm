import { Database } from "./cloudflare.js";

let lastSql = "";
let lastBind: unknown[] = [];

const fakeD1 = {
    prepare(query: string) {
        lastSql = query;
        return {
            bind(...values: unknown[]) {
                lastBind = values;
                return {
                    async all<T>() {
                        return {
                            results: [{ user_id: "u1", user_name: "Ada", active: 1 }] as T[],
                        };
                    },
                };
            },
        };
    },
};

const db = await Database.create({ d1: fakeD1 });
const unfiltered = await db.user.findMany();
const filtered = await db.user.findMany({ where: { active: { eq: true } } });
await db.$close();

console.log(
    JSON.stringify({
        ok: true,
        unfilteredCount: unfiltered.length,
        filteredCount: filtered.length,
        filteredSql: lastSql,
        filteredBind: lastBind,
    }),
);
