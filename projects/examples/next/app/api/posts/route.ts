import { createPost, listPosts, listPostsByAuthorName, openIrisDb } from "../../../lib/iris.ts";

export const runtime = "nodejs";

export async function GET(request: Request) {
    const author = new URL(request.url).searchParams.get("author")?.trim();
    const db = await openIrisDb();
    if (author) {
        return Response.json(await listPostsByAuthorName(db, author));
    }
    return Response.json(await listPosts(db));
}

export async function POST(request: Request) {
    const body = (await request.json()) as { author_user_id?: string; title?: string; published?: boolean };
    const authorUserId = body.author_user_id?.trim() ?? "";
    const title = body.title?.trim() ?? "";
    if (!authorUserId || !title) {
        return Response.json({ error: "author_user_id and title are required" }, { status: 400 });
    }
    const db = await openIrisDb();
    const post = await createPost(db, {
        author_user_id: authorUserId,
        title,
        published: body.published,
    });
    return Response.json(post, { status: 201 });
}
