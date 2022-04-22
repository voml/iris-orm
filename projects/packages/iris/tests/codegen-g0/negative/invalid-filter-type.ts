import { createClient } from "../../index.js";
import type { IrisDbBinding } from "@yydb/iris/types";

declare const binding: IrisDbBinding;
const db = createClient(binding);

async function run() {
    await db.post.findMany({
        where: {
            title: { contains: 123 },
        },
    });
}

void run;
