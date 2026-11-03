import { Client } from "../../operations.js";
import type { OperationExecutor } from "@yydb/iris/types";
import type { PostFindManyArgs } from "../../inputs.js";

declare const executor: OperationExecutor;
const db = new Client(executor);

async function run() {
    const args = {
        select: {
            postId: true,
            not_a_field: true,
        },
    } satisfies PostFindManyArgs;

    await db.tables.post.findMany(args);
}

void run;
