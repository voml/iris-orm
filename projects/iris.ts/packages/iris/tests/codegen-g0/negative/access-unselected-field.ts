import { Client } from "../../operations.js";
import type { OperationExecutor } from "@yydb/iris/types";

declare const executor: OperationExecutor;
const db = new Client(executor);

async function run() {
    const posts = await db.tables.post.findMany({
        select: { title: true },
    });
    const missing: string = posts[0]!.postId;
    void missing;
}

void run;
