declare const __irisUserIdBrand: unique symbol;
/** Branded primary key for User. */
export type UserId = string & {
    readonly [__irisUserIdBrand]: true;
};
/** Convert an external scalar into this schema's branded identity. */
export function userId(value: string): UserId {
    return value as UserId;
}
/** Stored reference identity for &User (not an ORM relation object). */
export type UserReference = {
    readonly __irisRef: "User";
    userId: UserId;
};
/** Values accepted when writing &User. */
export type UserRefInput =
    | UserId
    | { userId: UserId }
    | UserReference;
declare const __irisPostIdBrand: unique symbol;
/** Branded primary key for Post. */
export type PostId = string & {
    readonly [__irisPostIdBrand]: true;
};
/** Convert an external scalar into this schema's branded identity. */
export function postId(value: string): PostId {
    return value as PostId;
}
/** Stored reference identity for &Post (not an ORM relation object). */
export type PostReference = {
    readonly __irisRef: "Post";
    postId: PostId;
};
/** Values accepted when writing &Post. */
export type PostRefInput =
    | PostId
    | { postId: PostId }
    | PostReference;
