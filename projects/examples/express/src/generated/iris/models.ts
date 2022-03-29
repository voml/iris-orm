import type { PostId, PostReference, UserId, UserReference } from "./references.js";
export interface User {
    userId: UserId;
    userName: string;
    active: boolean;
}
export interface Post {
    postId: PostId;
    author: UserReference;
    title: string;
    published: boolean;
}
