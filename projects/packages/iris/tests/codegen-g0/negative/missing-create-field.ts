import { createClient } from "../../index.js";
import { userId } from "../../references.js";
import type { IrisDbBinding } from "@yydb/iris/types";

declare const binding: IrisDbBinding;
const db = createClient(binding);

async function run() {
    await db.user.create({
        data: {
            user_id: userId("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"),
            active: true,
        },
        select: { user_id: true },
    });
}

void run;
