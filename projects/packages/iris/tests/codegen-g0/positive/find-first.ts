import { createClient } from "../../index.js";
import type { IrisDbBinding } from "@yydb/iris/types";

declare const binding: IrisDbBinding;
const db = createClient(binding);

async function run() {
    const first = await db.post.findFirst({
        where: { published: true },
        select: { post_id: true, title: true },
    });
    if (first) {
        const title: string = first.title;
        void title;
    }
}

void run;
