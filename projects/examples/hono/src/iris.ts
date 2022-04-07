import { randomUUID } from "node:crypto";
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { createIrisDbBinding } from "@yydb/iris/node";
import type { IrisDbBinding } from "@yydb/iris/types";

export type User = {
    user_id: string;
    user_name: string;
    active: boolean;
};

const schemaPath = join(dirname(fileURLToPath(import.meta.url)), "../schemas/user.iris");

let binding: IrisDbBinding | null = null;

async function loadSchema(): Promise<string> {
    return readFile(schemaPath, "utf8");
}

export async function openIrisDb(): Promise<IrisDbBinding> {
    if (binding) {
        return binding;
    }
    binding = await createIrisDbBinding({
        profile: "sqlite",
        sqlitePath: ":memory:",
        schema: await loadSchema(),
    });
    await seedUsers(binding);
    return binding;
}

export async function closeIrisDb(): Promise<void> {
    if (!binding) {
        return;
    }
    await binding.close();
    binding = null;
}

export async function listUsers(db: IrisDbBinding): Promise<User[]> {
    const rows = await db.query("User.filter(x => x.active).collect()");
    return rows as User[];
}

export async function createUser(db: IrisDbBinding, userName: string, active = true): Promise<User> {
    const user: User = {
        user_id: randomUUID(),
        user_name: userName,
        active,
    };
    await db.execute("User::insert({ user_id: $user_id, user_name: $user_name, active: $active })", {
        user_id: user.user_id,
        user_name: user.user_name,
        active: user.active,
    });
    return user;
}

async function seedUsers(db: IrisDbBinding): Promise<void> {
    if ((await listUsers(db)).length > 0) {
        return;
    }
    await createUser(db, "ada", true);
    await createUser(db, "linus", true);
}
