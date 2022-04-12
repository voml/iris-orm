import { randomUUID } from "node:crypto";
import { json, type RequestEvent } from "@sveltejs/kit";

import { getDb } from "$lib/server/db.ts";
import type { PostId, UserId } from "../../../generated/iris/references.ts";

const postListSelect = {
    post_id: true,
    title: true,
    published: true,
    author: { user_name: true },
} as const;

export async function GET({ url }: RequestEvent) {
    const author = url.searchParams.get("author")?.trim();
    const db = await getDb();
    const where = author
        ? { published: true, author: { user_name: author } }
        : { published: true };
    return json(await db.post.findMany({ where, select: postListSelect }));
}

export async function POST({ request }: RequestEvent) {
    const body = (await request.json()) as { author_user_id?: string; title?: string; published?: boolean };
    const authorUserId = body.author_user_id?.trim() ?? "";
    const title = body.title?.trim() ?? "";
    if (!authorUserId || !title) {
        return json({ error: "author_user_id and title are required" }, { status: 400 });
    }
    const db = await getDb();
    const post = await db.post.create({
        data: {
            post_id: randomUUID() as PostId,
            author: authorUserId as UserId,
            title,
            published: body.published ?? true,
        },
    });
    return json(post, { status: 201 });
}
