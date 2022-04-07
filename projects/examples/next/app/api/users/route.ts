import { createUser, listUsers, openIrisDb } from "../../../lib/iris.ts";

export const runtime = "nodejs";

export async function GET() {
    const db = await openIrisDb();
    return Response.json(await listUsers(db));
}

export async function POST(request: Request) {
    const body = (await request.json()) as { user_name?: string; active?: boolean };
    const userName = body.user_name?.trim() ?? "";
    if (!userName) {
        return Response.json({ error: "user_name is required" }, { status: 400 });
    }
    const db = await openIrisDb();
    const user = await createUser(db, userName, body.active ?? true);
    return Response.json(user, { status: 201 });
}
