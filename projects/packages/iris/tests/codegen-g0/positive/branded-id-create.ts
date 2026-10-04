import { createClient } from "../../index.js";
import { postId, userId } from "../../references.js";
import type { OperationExecutor } from "@yydb/iris/types";

declare const executor: OperationExecutor;
const db = createClient(executor);

async function run() {
    const author = await db.user.create({
        data: {
            userId: userId("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"),
            userName: "Ada",
            active: true,
        },
        select: { userId: true, userName: true },
    });

    await db.post.create({
        data: {
            postId: postId("bbbbbbbb-cccc-dddd-eeee-ffff00000000"),
            author: author.userId,
            title: "Hello Iris",
            published: true,
        },
        select: { postId: true, title: true },
    });
}

void run;
