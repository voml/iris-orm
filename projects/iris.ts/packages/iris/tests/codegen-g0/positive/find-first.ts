import { createClient } from "../../index.js";
import type { OperationExecutor } from "@yydb/iris/types";

declare const executor: OperationExecutor;
const db = createClient(executor);

async function run() {
    const first = await db.post.findFirst({
        where: { published: true },
        select: { postId: true, title: true },
    });
    if (first) {
        const title: string = first.title;
        void title;
    }
}

void run;
