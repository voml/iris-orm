/**
 * Thin D1 access layer — proves the `DB` binding until Iris ships a D1 / foreign-sqlite session.
 * Shapes align with `@iris/index.ts` domain types; replace with `createDatabase({ binding: env.DB })` later.
 */

export type D1UserRow = {
    user_id: string;
    user_name: string;
    active: number;
};

export type D1PostRow = {
    post_id: string;
    author: string;
    title: string;
    published: number;
    author_name?: string;
};

export async function d1ListActiveUsers(db: D1Database): Promise<D1UserRow[]> {
    const { results } = await db
        .prepare("SELECT user_id, user_name, active FROM User WHERE active = 1 ORDER BY user_name")
        .all<D1UserRow>();
    return results ?? [];
}

export async function d1ListPublishedPosts(db: D1Database, authorName?: string): Promise<D1PostRow[]> {
    if (authorName?.trim()) {
        const { results } = await db
            .prepare(
                `SELECT p.post_id, p.author, p.title, p.published, u.user_name AS author_name
                 FROM Post p
                 INNER JOIN User u ON u.user_id = p.author
                 WHERE p.published = 1 AND u.user_name = ?
                 ORDER BY p.title`,
            )
            .bind(authorName.trim())
            .all<D1PostRow>();
        return results ?? [];
    }
    const { results } = await db
        .prepare(
            `SELECT p.post_id, p.author, p.title, p.published, u.user_name AS author_name
             FROM Post p
             INNER JOIN User u ON u.user_id = p.author
             WHERE p.published = 1
             ORDER BY p.title`,
        )
        .all<D1PostRow>();
    return results ?? [];
}

export async function d1InsertUser(
    db: D1Database,
    row: { user_id: string; user_name: string; active: boolean },
): Promise<void> {
    await db
        .prepare("INSERT INTO User (user_id, user_name, active) VALUES (?, ?, ?)")
        .bind(row.user_id, row.user_name, row.active ? 1 : 0)
        .run();
}

export async function d1InsertPost(
    db: D1Database,
    row: { post_id: string; author: string; title: string; published: boolean },
): Promise<void> {
    await db
        .prepare("INSERT INTO Post (post_id, author, title, published) VALUES (?, ?, ?, ?)")
        .bind(row.post_id, row.author, row.title, row.published ? 1 : 0)
        .run();
}
