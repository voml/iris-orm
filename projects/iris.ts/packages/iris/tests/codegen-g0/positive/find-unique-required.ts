import { Client } from "../../operations.js";
import { postId } from "../../references.js";
import type { OperationExecutor } from "@yydb/iris/types";

declare const executor: OperationExecutor;
const db = new Client(executor);

async function run() {
    const post = await db.tables.post.findUnique({
        where: {
            postId: postId("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"),
        },
        select: { title: true },
    });
    if (post) {
        const title: string = post.title;
        void title;
    }
}

void run;
