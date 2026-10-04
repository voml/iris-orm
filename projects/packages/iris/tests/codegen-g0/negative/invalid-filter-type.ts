import { createClient } from "../../index.js";
import type { OperationExecutor } from "@yydb/iris/types";

declare const executor: OperationExecutor;
const db = createClient(executor);

async function run() {
    await db.post.findMany({
        where: {
            title: { contains: 123 },
        },
    });
}

void run;
