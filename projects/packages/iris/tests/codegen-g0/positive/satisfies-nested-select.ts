import { createClient } from "../../index.js";
import type { IrisDbBinding } from "@yydb/iris/types";
import type { PostFindManyArgs } from "../../inputs.js";

declare const binding: IrisDbBinding;
const db = createClient(binding);

async function run() {
    const args = {
        where: {
            published: true,
            author: {
                is: {
                    user_name: { contains: "Ada" },
                },
            },
        },
        select: {
            post_id: true,
            title: true,
            author: {
                select: {
                    user_id: true,
                    user_name: true,
                },
            },
        },
    } satisfies PostFindManyArgs;

    const posts = await db.post.findMany(args);
    const title: string = posts[0]!.title;
    const authorName: string = posts[0]!.author.user_name;
    void title;
    void authorName;
}

void run;
