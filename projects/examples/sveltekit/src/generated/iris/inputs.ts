import type {
    User,
    Post,
} from "./models.js";
import type {
    UserId,
    UserRefInput,
    PostId,
    PostRefInput,
} from "./references.js";

/** Shared input/result type helpers for generated delegates (use-site reference paths). */

type EntityByName = {
    User: User;
    Post: Post;
};

type EntityName = "User" | "Post";type SelectShape<S> = S extends { readonly select: infer Nested } ? Nested : S;

/** Scalar filter operators for utf8 fields. */
export type StringFilter = {
    readonly eq?: string;
    readonly not?: string;
    readonly contains?: string;
    readonly startsWith?: string;
    readonly endsWith?: string;
};

/** Scalar filter operators for numeric fields. */
export type NumberFilter = {
    readonly eq?: number;
    readonly not?: number;
    readonly gt?: number;
    readonly gte?: number;
    readonly lt?: number;
    readonly lte?: number;
};

/** Scalar filter operators for bool fields. */
export type BooleanFilter = {
    readonly eq?: boolean;
    readonly not?: boolean;
};

/** Scalar filter operators for uuid / branded id fields. */
export type UuidFilter = {
    readonly eq?: string;
    readonly not?: string;
};

/** Scalar filter operators for decimal fields. */
export type DecimalFilter = {
    readonly eq?: string;
    readonly not?: string;
    readonly gt?: string;
    readonly gte?: string;
    readonly lt?: string;
    readonly lte?: string;
};

/** Scalar filter operators for datetime fields. */
export type DateTimeFilter = {
    readonly eq?: string;
    readonly not?: string;
    readonly gt?: string;
    readonly gte?: string;
    readonly lt?: string;
    readonly lte?: string;
};

/** Scalar filter operators for bytes fields. */
export type BytesFilter = {
    readonly eq?: string;
    readonly not?: string;
};

/** Patch field: write a value. Omitted field means unchanged. */
export type PatchValue<T> = { readonly set: T };

/** Patch field on nullable columns: write a value or explicit null. */
export type NullablePatchValue<T> = PatchValue<T> | { readonly setNull: true };

/** Nested select shape for a reference target (use-site dereference projection). */
export type SelectPathFor<E extends EntityName> = {
    [K in keyof EntityByName[E]]?:
        | boolean
        | (EntityByName[E][K] extends { readonly __irisRef: infer T extends EntityName }
              ? { readonly select: SelectPathFor<T> }
              : never);
};

/** Nested where path for a reference target (use-site filter navigation). */
export type WherePathFor<E extends EntityName> = {
    [K in keyof EntityByName[E]]?:
        | WhereValueForField<EntityByName[E][K]>
        | (EntityByName[E][K] extends { readonly __irisRef: infer T extends EntityName }
              ? WherePathFor<T> | { readonly is: WherePathFor<T> }
              : never);
};

type WhereValueForField<V> = V extends { readonly __irisRef: EntityName }
    ? never
    : V extends boolean
      ? boolean | BooleanFilter | { readonly isNull: true } | { readonly isNotNull: true }
      : V extends number
        ? number | NumberFilter | { readonly isNull: true } | { readonly isNotNull: true }
        : V extends string
          ? string | StringFilter | UuidFilter | DecimalFilter | DateTimeFilter | BytesFilter | { readonly isNull: true } | { readonly isNotNull: true }
          : never;

/**
 * Resolve a select shape against an entity — nested object on &T becomes
 * SelectResultFor of the target; `true` on &T means full target entity.
 */
export type SelectResultFor<E extends EntityName, S> = [S] extends [undefined]
    ? EntityByName[E]
    : {
          [K in keyof SelectShape<S> & keyof EntityByName[E] as SelectShape<S>[K] extends true | object ? K : never]: SelectShape<S>[K] extends true
              ? EntityByName[E][K] extends { readonly __irisRef: infer T extends EntityName }
                  ? EntityByName[T]
                  : EntityByName[E][K]
              : SelectShape<S>[K] extends object
                ? EntityByName[E][K] extends { readonly __irisRef: infer T extends EntityName }
                    ? SelectResultFor<T, SelectShape<S>[K]>
                    : never
                : never;
      };



export type UserSelect = {

    userId?: boolean;


    userName?: boolean;


    active?: boolean;


};

export type UserWhereInput = {

    userId?: UserId | UuidFilter;


    userName?: string | StringFilter;


    active?: boolean | BooleanFilter;


};

export type UserWhereUniqueInput = {
    userId: UserId;
};

export type UserCreateInput = {

    userId: UserId;


    userName: string;


    active: boolean;


};

export type UserPatchInput = {



    userName?: PatchValue<string>;


    active?: PatchValue<boolean>;


};

export type UserFindManyArgs = {
    where?: UserWhereInput;
    select?: UserSelect;
    take?: number;
};

export type UserFindUniqueArgs = {
    where: UserWhereUniqueInput;
    select?: UserSelect;
};export type UserFindFirstArgs = UserFindManyArgs;

export type UserCreateArgs = {
    data: UserCreateInput;
    select?: UserSelect;
};

export type UserGetPayload<A extends UserFindManyArgs | UserFindUniqueArgs | UserCreateArgs> =
    SelectResultFor<"User", A["select"]>;


export type PostSelect = {

    postId?: boolean;


    author?: boolean | { readonly select: SelectPathFor<"User"> };


    title?: boolean;


    published?: boolean;


};

export type PostWhereInput = {

    postId?: PostId | UuidFilter;


    author?: WherePathFor<"User"> | { readonly is: WherePathFor<"User"> };


    title?: string | StringFilter;


    published?: boolean | BooleanFilter;


};

export type PostWhereUniqueInput = {
    postId: PostId;
};

export type PostCreateInput = {

    postId: PostId;


    author: UserRefInput;


    title: string;


    published: boolean;


};

export type PostPatchInput = {



    author?: PatchValue<UserRefInput>;


    title?: PatchValue<string>;


    published?: PatchValue<boolean>;


};

export type PostFindManyArgs = {
    where?: PostWhereInput;
    select?: PostSelect;
    take?: number;
};

export type PostFindUniqueArgs = {
    where: PostWhereUniqueInput;
    select?: PostSelect;
};export type PostFindFirstArgs = PostFindManyArgs;

export type PostCreateArgs = {
    data: PostCreateInput;
    select?: PostSelect;
};

export type PostGetPayload<A extends PostFindManyArgs | PostFindUniqueArgs | PostCreateArgs> =
    SelectResultFor<"Post", A["select"]>;
