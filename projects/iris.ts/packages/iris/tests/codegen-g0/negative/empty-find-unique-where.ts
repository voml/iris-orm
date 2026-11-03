import { Client } from "../../operations.js";
import type { OperationExecutor } from "@yydb/iris/types";

declare const executor: OperationExecutor;
const db = new Client(executor);

async function run() {
    await db.tables.post.findUnique({
        where: {},
        select: { title: true },
    });
}

void run;
