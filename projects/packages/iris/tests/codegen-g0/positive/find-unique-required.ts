import { createClient } from "../../index.js";
import { postId } from "../../references.js";
import type { IrisDbBinding } from "@yydb/iris/types";

declare const binding: IrisDbBinding;
const db = createClient(binding);

async function run() {
    const post = await db.post.findUnique({
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
