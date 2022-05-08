import { createClient } from "../../index.js";
import { postId, userId } from "../../references.js";
import type { IrisDbBinding } from "@yydb/iris/types";
import type { PostId } from "../../references.js";

declare const binding: IrisDbBinding;
const db = createClient(binding);

async function run() {
    const wrong: PostId = userId("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
    await db.post.findUnique({
        where: { post_id: wrong },
        select: { title: true },
    });
    await db.post.findUnique({
        where: { post_id: postId("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee") },
        select: { title: true },
    });
}

void run;
