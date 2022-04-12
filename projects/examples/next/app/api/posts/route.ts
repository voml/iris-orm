import { randomUUID } from "node:crypto";

import type { PostId, UserId } from "@iris/index.ts";
import { createDb } from "@iris/node.ts";

export const runtime = "nodejs";

const postListSelect = {
    post_id: true,
    title: true,
    published: true,
    author: { user_name: true },
} as const;

export async function GET(request: Request) {
    const author = new URL(request.url).searchParams.get("author")?.trim();
    const db = await createDb({ config: process.cwd(), source: "default" });
    const where = author
        ? { published: true, author: { user_name: author } }
        : { published: true };
    return Response.json(await db.post.findMany({ where, select: postListSelect }));
}

export async function POST(request: Request) {
    const body = (await request.json()) as { author_user_id?: string; title?: string; published?: boolean };
    const authorUserId = body.author_user_id?.trim() ?? "";
    const title = body.title?.trim() ?? "";
    if (!authorUserId || !title) {
        return Response.json({ error: "author_user_id and title are required" }, { status: 400 });
    }
    const db = await createDb({ config: process.cwd(), source: "default" });
    const post = await db.post.create({
        data: {
            post_id: randomUUID() as PostId,
            author: authorUserId as UserId,
            title,
            published: body.published ?? true,
        },
    });
    return Response.json(post, { status: 201 });
}
