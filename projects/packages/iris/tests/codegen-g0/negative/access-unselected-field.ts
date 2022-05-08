import { createClient } from "../../index.js";
import type { IrisDbBinding } from "@yydb/iris/types";

declare const binding: IrisDbBinding;
const db = createClient(binding);

async function run() {
    const posts = await db.post.findMany({
        select: { title: true },
    });
    const missing: string = posts[0]!.post_id;
    void missing;
}

void run;
