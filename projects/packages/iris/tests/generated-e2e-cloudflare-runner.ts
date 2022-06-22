import { Database } from "./cloudflare.js";

const fakeD1 = {
    prepare(query: string) {
        return {
            bind() {
                return {
                    async all<T>() {
                        void query;
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
const users = await db.user.findMany();
await db.$close();

console.log(
    JSON.stringify({
        ok: true,
        count: users.length,
        userId: users[0]?.userId,
        userName: users[0]?.userName,
    }),
);
