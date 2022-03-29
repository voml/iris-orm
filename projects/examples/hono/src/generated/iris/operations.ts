import type { IrisDbBinding } from "@yydb/iris/types";
import { synthesizeCreate, synthesizeFindFirst, synthesizeFindMany, synthesizeFindUnique, synthesizeMacroCall } from "./_internal/synthesize.js";
import type {
    UserFindManyArgs, UserFindFirstArgs, UserFindUniqueArgs, UserCreateArgs, UserGetPayload, PostFindManyArgs, PostFindFirstArgs, PostFindUniqueArgs, PostCreateArgs, PostGetPayload
} from "./inputs.js";
/** Generated macro bindings from schema truth (not backend capability intersection). */
export class GeneratedMacros {
    constructor(private readonly binding: IrisDbBinding) {}
    async seed_blog(): Promise<void> {
        const { source, parameters } = synthesizeMacroCall("seed_blog", []);
        await this.binding.execute(source, parameters);
    }
}
export class UserDelegate {
    constructor(private readonly binding: IrisDbBinding) {}
    async findMany<const A extends UserFindManyArgs>(
        args?: A,
    ): Promise<ReadonlyArray<UserGetPayload<A>>> {
        const { source, parameters } = synthesizeFindMany("User", args);
        const rows = await this.binding.query(source, parameters);
        return rows as ReadonlyArray<UserGetPayload<A>>;
    }
    async findFirst<const A extends UserFindFirstArgs>(
        args?: A,
    ): Promise<UserGetPayload<A> | null> {
        const { source, parameters } = synthesizeFindFirst("User", args);
        const rows = (await this.binding.query(source, parameters)) as unknown[];
        return (rows[0] ?? null) as UserGetPayload<A> | null;
    }
    async findUnique<const A extends UserFindUniqueArgs>(
        args: A,
    ): Promise<UserGetPayload<A> | null> {
        const { source, parameters } = synthesizeFindUnique("User", args);
        const rows = (await this.binding.query(source, parameters)) as unknown[];
        return (rows[0] ?? null) as UserGetPayload<A> | null;
    }
    async create<const A extends UserCreateArgs>(
        args: A,
    ): Promise<UserGetPayload<A>> {
        const { source, parameters } = synthesizeCreate("User", args);
        const rows = (await this.binding.query(source, parameters)) as unknown[];
        return rows[0] as UserGetPayload<A>;
    }
}
export class PostDelegate {
    constructor(private readonly binding: IrisDbBinding) {}
    async findMany<const A extends PostFindManyArgs>(
        args?: A,
    ): Promise<ReadonlyArray<PostGetPayload<A>>> {
        const { source, parameters } = synthesizeFindMany("Post", args);
        const rows = await this.binding.query(source, parameters);
        return rows as ReadonlyArray<PostGetPayload<A>>;
    }
    async findFirst<const A extends PostFindFirstArgs>(
        args?: A,
    ): Promise<PostGetPayload<A> | null> {
        const { source, parameters } = synthesizeFindFirst("Post", args);
        const rows = (await this.binding.query(source, parameters)) as unknown[];
        return (rows[0] ?? null) as PostGetPayload<A> | null;
    }
    async findUnique<const A extends PostFindUniqueArgs>(
        args: A,
    ): Promise<PostGetPayload<A> | null> {
        const { source, parameters } = synthesizeFindUnique("Post", args);
        const rows = (await this.binding.query(source, parameters)) as unknown[];
        return (rows[0] ?? null) as PostGetPayload<A> | null;
    }
    async create<const A extends PostCreateArgs>(
        args: A,
    ): Promise<PostGetPayload<A>> {
        const { source, parameters } = synthesizeCreate("Post", args);
        const rows = (await this.binding.query(source, parameters)) as unknown[];
        return rows[0] as PostGetPayload<A>;
    }
}
export class DbClient {
    readonly user: UserDelegate;
    readonly post: PostDelegate;
    readonly $macros: GeneratedMacros;
    private readonly binding: IrisDbBinding;
    constructor(binding: IrisDbBinding) {
        this.binding = binding;
        this.$macros = new GeneratedMacros(binding);
        this.user = new UserDelegate(binding);
        this.post = new PostDelegate(binding);
    }
    /**
     * Escape hatch for ad-hoc VOS. Prefer generated delegates for typed args and results.
     * @internal
     */
    async $query<T = unknown>(
        source: string,
        parameters?: Readonly<Record<string, unknown>>,
    ): Promise<T> {
        return (await this.binding.query(source, parameters)) as T;
    }
    /** Direct VOS DDL. Success maps VOS unit to void. */
    async $execute(
        source: string,
        parameters?: Readonly<Record<string, unknown>>,
    ): Promise<void> {
        await this.binding.execute(source, parameters);
    }
    async $close(): Promise<void> {
        await this.binding.close();
    }
}
/** Construct a client from an injected binding (Node/Web host wiring stays outside generated types). */
export function createClient(binding: IrisDbBinding): DbClient {
    return new DbClient(binding);
}
