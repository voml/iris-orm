import { createClient } from "../../index.js";
import type { OperationExecutor } from "@yydb/iris/types";
import type { PostFindManyArgs } from "../../inputs.js";

declare const executor: OperationExecutor;
const db = createClient(executor);

async function run() {
    const args = {
        select: {
            post_id: true,
        },
    } satisfies PostFindManyArgs;

    await db.post.findMany(args);
}

void run;
