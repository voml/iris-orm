import { randomUUID } from "node:crypto";
import type { APIRoute } from "astro";

import { Database } from "@iris/node.ts";
import type { PostId, UserId } from "@iris/index.ts";

const postListSelect = {
    post_id: true,
    title: true,
    published: true,
    author: { select: { user_name: true } },
} as const;

export const GET: APIRoute = async ({ request }) => {
    const author = new URL(request.url).searchParams.get("author")?.trim();
    const db = await Database.open({ config: process.cwd(), source: "default" });
    const where = author
        ? { published: true, author: { user_name: author } }
        : { published: true };
    return new Response(JSON.stringify(await db.post.findMany({ where, select: postListSelect })), {
        headers: { "content-type": "application/json" },
    });
};

export const POST: APIRoute = async ({ request }) => {
    const body = (await request.json()) as { author_user_id?: string; title?: string; published?: boolean };
    const authorUserId = body.author_user_id?.trim() ?? "";
    const title = body.title?.trim() ?? "";
    if (!authorUserId || !title) {
        return new Response(JSON.stringify({ error: "author_user_id and title are required" }), {
            status: 400,
            headers: { "content-type": "application/json" },
        });
    }
    const db = await Database.open({ config: process.cwd(), source: "default" });
    const post = await db.post.create({
        data: {
            post_id: randomUUID() as PostId,
            author: authorUserId as UserId,
            title,
            published: body.published ?? true,
        },
    });
    return new Response(JSON.stringify(post), {
        status: 201,
        headers: { "content-type": "application/json" },
    });
};
