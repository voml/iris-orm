import { createClient } from "../../index.js";
import type { IrisDbBinding } from "@yydb/iris/types";
import type { PostFindManyArgs } from "../../inputs.js";

declare const binding: IrisDbBinding;
const db = createClient(binding);

async function run() {
    const args = {
        select: {
            post_id: true,
        },
    } satisfies PostFindManyArgs;

    await db.post.findMany(args);
}

void run;
