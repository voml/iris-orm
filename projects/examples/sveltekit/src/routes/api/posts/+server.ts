import { json } from "@sveltejs/kit";

import { createPost, listPosts, listPostsByAuthorName, openIrisDb } from "$lib/iris.ts";
import type { RequestHandler } from "./$types";

export const GET: RequestHandler = async ({ url }) => {
    const author = url.searchParams.get("author")?.trim();
    const db = await openIrisDb();
    if (author) {
        return json(await listPostsByAuthorName(db, author));
    }
    return json(await listPosts(db));
};

export const POST: RequestHandler = async ({ request }) => {
    const body = (await request.json()) as { author_user_id?: string; title?: string; published?: boolean };
    const authorUserId = body.author_user_id?.trim() ?? "";
    const title = body.title?.trim() ?? "";
    if (!authorUserId || !title) {
        return json({ error: "author_user_id and title are required" }, { status: 400 });
    }
    const db = await openIrisDb();
    const post = await createPost(db, {
        author_user_id: authorUserId,
        title,
        published: body.published,
    });
    return json(post, { status: 201 });
};
