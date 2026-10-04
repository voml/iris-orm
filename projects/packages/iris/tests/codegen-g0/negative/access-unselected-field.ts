import { createClient } from "../../index.js";
import type { OperationExecutor } from "@yydb/iris/types";

declare const executor: OperationExecutor;
const db = createClient(executor);

async function run() {
    const posts = await db.post.findMany({
        select: { title: true },
    });
    const missing: string = posts[0]!.postId;
    void missing;
}

void run;
