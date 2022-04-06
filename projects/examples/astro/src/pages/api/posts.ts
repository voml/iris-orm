import type { APIRoute } from "astro";

import { createPost, listPosts, listPostsByAuthorName, openIrisDb } from "../../lib/iris.ts";

export const GET: APIRoute = async ({ url }) => {
    const author = url.searchParams.get("author")?.trim();
    const db = await openIrisDb();
    const rows = author ? await listPostsByAuthorName(db, author) : await listPosts(db);
    return new Response(JSON.stringify(rows), {
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
    const db = await openIrisDb();
    const post = await createPost(db, {
        author_user_id: authorUserId,
        title,
        published: body.published,
    });
    return new Response(JSON.stringify(post), {
        status: 201,
        headers: { "content-type": "application/json" },
    });
};
