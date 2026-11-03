import { Client } from "../../operations.js";
import { postId, userId } from "../../references.js";
import type { OperationExecutor } from "@yydb/iris/types";
import type { PostId } from "../../references.js";

declare const executor: OperationExecutor;
const db = new Client(executor);

async function run() {
    const wrong: PostId = userId("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
    await db.tables.post.findUnique({
        where: { post_id: wrong },
        select: { title: true },
    });
    await db.tables.post.findUnique({
        where: { post_id: postId("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee") },
        select: { title: true },
    });
}

void run;
