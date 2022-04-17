import { randomUUID } from "node:crypto";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { createIrisDbBinding } from "@yydb/iris/node";
import type { IrisDbBinding } from "@yydb/iris/types";

export type User = {
    user_id: string;
    user_name: string;
    active: boolean;
};

/** Post row projected through `author: &User` — note `author_name` from `x.author.user_name`. */
export type PostSummary = {
    post_id: string;
    title: string;
    author_name: string;
    published: boolean;
};

const projectRoot = join(dirname(fileURLToPath(import.meta.url)), "..");

let binding: IrisDbBinding | null = null;

export async function openIrisDb(): Promise<IrisDbBinding> {
    if (binding) {
        return binding;
    }
    binding = await createIrisDbBinding({
        config: projectRoot,
        source: "default",
    });
    await seedBlog(binding);
    return binding;
}

export async function closeIrisDb(): Promise<void> {
    if (!binding) {
        return;
    }
    await binding.close();
    binding = null;
}

export async function listUsers(db: IrisDbBinding): Promise<User[]> {
    const rows = await db.query("User.filter(x => x.active).collect()");
    return rows as User[];
}

export async function createUser(db: IrisDbBinding, userName: string, active = true): Promise<User> {
    const user: User = {
        user_id: randomUUID(),
        user_name: userName,
        active,
    };
    await db.execute("User::insert({ user_id: $user_id, user_name: $user_name, active: $active })", {
        user_id: user.user_id,
        user_name: user.user_name,
        active: user.active,
    });
    return user;
}

export async function listPosts(db: IrisDbBinding): Promise<PostSummary[]> {
    const rows = await db.query(
        "Post.filter(x => x.published).map(x => { post_id: x.post_id, title: x.title, author_name: x.author.user_name, published: x.published }).collect()",
    );
    return rows as PostSummary[];
}

/** Filter posts by traversing the `&User` reference in VOS (`x.author.user_name`). */
export async function listPostsByAuthorName(db: IrisDbBinding, authorName: string): Promise<PostSummary[]> {
    const rows = await db.query(
        "Post.filter(x => x.author.user_name == $name).map(x => { post_id: x.post_id, title: x.title, author_name: x.author.user_name, published: x.published }).collect()",
        { name: authorName },
    );
    return rows as PostSummary[];
}

/** Insert a post; `author_user_id` is the FK stored for `author: &User`. */
export async function createPost(
    db: IrisDbBinding,
    input: { author_user_id: string; title: string; published?: boolean },
): Promise<{ post_id: string; author_user_id: string; title: string; published: boolean }> {
    const post = {
        post_id: randomUUID(),
        author_user_id: input.author_user_id,
        title: input.title,
        published: input.published ?? true,
    };
    await db.execute("Post::insert({ post_id: $post_id, author: $author, title: $title, published: $published })", {
        post_id: post.post_id,
        author: post.author_user_id,
        title: post.title,
        published: post.published,
    });
    return post;
}

async function seedBlog(db: IrisDbBinding): Promise<void> {
    if ((await listUsers(db)).length > 0) {
        return;
    }
    const ada = await createUser(db, "ada", true);
    const linus = await createUser(db, "linus", true);
    await createPost(db, {
        author_user_id: ada.user_id,
        title: "Hello &User refs",
        published: true,
    });
    await createPost(db, {
        author_user_id: linus.user_id,
        title: "Filter via x.author.user_name",
        published: true,
    });
}
