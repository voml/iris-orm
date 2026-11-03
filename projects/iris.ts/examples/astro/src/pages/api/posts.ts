import { randomUUID } from "node:crypto";
import type { APIRoute } from "astro";

import { createClient } from "@iris/node.ts";
import type { PostId, UserId } from "@iris/index.ts";

const postListSelect = {
    postId: true,
    title: true,
    published: true,
    author: { select: { userName: true } },
} as const;

export const GET: APIRoute = async ({ request }) => {
    const author = new URL(request.url).searchParams.get("author")?.trim();
    const db = createClient({ dsn: "file:.iris/dev.yydb" });
    const where = author
        ? { published: true, author: { userName: author } }
        : { published: true };
    return new Response(JSON.stringify(await db.tables.post.findMany({ where, select: postListSelect })), {
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
    const db = createClient({ dsn: "file:.iris/dev.yydb" });
    const post = await db.tables.post.create({
        data: {
            postId: randomUUID() as PostId,
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
