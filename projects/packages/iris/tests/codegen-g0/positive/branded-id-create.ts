import { createClient } from "../../index.js";
import { postId, userId } from "../../references.js";
import type { IrisDbBinding } from "@yydb/iris/types";

declare const binding: IrisDbBinding;
const db = createClient(binding);

async function run() {
    const author = await db.user.create({
        data: {
            user_id: userId("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"),
            user_name: "Ada",
            active: true,
        },
        select: { user_id: true, user_name: true },
    });

    await db.post.create({
        data: {
            post_id: postId("bbbbbbbb-cccc-dddd-eeee-ffff00000000"),
            author: author.user_id,
            title: "Hello Iris",
            published: true,
        },
        select: { post_id: true, title: true },
    });
}

void run;
