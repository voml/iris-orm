import { createClient } from "../../index.js";
import { userId } from "../../references.js";
import type { OperationExecutor } from "@yydb/iris/types";

declare const executor: OperationExecutor;
const db = createClient(executor);

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
