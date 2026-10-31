import type { OperationExecutor } from "./_internal/contract-types.js";
import {
    buildDeclaredOperationRequest,
    synthesizeCreate,
    synthesizeDelete,
    synthesizeUpdate,
    synthesizeFindFirst,
    synthesizeFindMany,
    synthesizeFindUnique,
    synthesizeOperationCall,
    throwOperationEnvelopeError,
} from "./_internal/synthesize.js";
import type {
    UserFindManyArgs, UserFindFirstArgs, UserFindUniqueArgs, UserCreateArgs, UserDeleteArgs, UserUpdateArgs, UserGetPayload, PostFindManyArgs, PostFindFirstArgs, PostFindUniqueArgs, PostCreateArgs, PostDeleteArgs, PostUpdateArgs, PostGetPayload
} from "./inputs.js";
/** Generated macro bindings from schema truth (not backend capability intersection). */
export class GeneratedMacros {
    private readonly executor: OperationExecutor;
    constructor(executor: OperationExecutor) {
        this.executor = executor;
    }
    async seed_blog(): Promise<void> {
        const synthesis = synthesizeOperationCall("seed_blog", []);
        const request = buildDeclaredOperationRequest("op_83d9146745765d0b7e4ffec3106540cec2437a2597e8a8faf1d044fb54dcb81e", synthesis);
        const envelope = await this.executor.executeUnit(request);
        if (!envelope.ok) {
            throwOperationEnvelopeError(envelope);
        }
    }
}
export class UserDelegate {
    private readonly executor: OperationExecutor;
    constructor(executor: OperationExecutor) {
        this.executor = executor;
    }
    async findMany<const A extends UserFindManyArgs>(
        args?: A,
    ): Promise<ReadonlyArray<UserGetPayload<A>>> {
        const synthesis = synthesizeFindMany("User", args);
        const envelope = await this.executor.execute(
            buildDeclaredOperationRequest("User.findMany", synthesis),
        );
        if (!envelope.ok) {
            throwOperationEnvelopeError(envelope);
        }
        return envelope.value as ReadonlyArray<UserGetPayload<A>>;
    }
    async findFirst<const A extends UserFindFirstArgs>(
        args?: A,
    ): Promise<UserGetPayload<A> | null> {
        const synthesis = synthesizeFindFirst("User", args);
        const envelope = await this.executor.execute(
            buildDeclaredOperationRequest("User.findFirst", synthesis),
        );
        if (!envelope.ok) {
            throwOperationEnvelopeError(envelope);
        }
        const rows = envelope.value;
        return (rows[0] ?? null) as UserGetPayload<A> | null;
    }
    async findUnique<const A extends UserFindUniqueArgs>(
        args: A,
    ): Promise<UserGetPayload<A> | null> {
        const synthesis = synthesizeFindUnique("User", args);
        const envelope = await this.executor.execute(
            buildDeclaredOperationRequest("User.findUnique", synthesis),
        );
        if (!envelope.ok) {
            throwOperationEnvelopeError(envelope);
        }
        const rows = envelope.value;
        return (rows[0] ?? null) as UserGetPayload<A> | null;
    }
    async create<const A extends UserCreateArgs>(
        args: A,
    ): Promise<UserGetPayload<A>> {
        const synthesis = synthesizeCreate("User", args);
        const envelope = await this.executor.execute(
            buildDeclaredOperationRequest("User.create", synthesis),
        );
        if (!envelope.ok) {
            throwOperationEnvelopeError(envelope);
        }
        return envelope.value[0] as UserGetPayload<A>;
    }
    async update<const A extends UserUpdateArgs>(
        args: A,
    ): Promise<UserGetPayload<A>> {
        const synthesis = synthesizeUpdate("User", args);
        const envelope = await this.executor.execute(
            buildDeclaredOperationRequest("User.update", synthesis),
        );
        if (!envelope.ok) {
            throwOperationEnvelopeError(envelope);
        }
        return envelope.value[0] as UserGetPayload<A>;
    }
    async delete(args: UserDeleteArgs): Promise<void> {
        const synthesis = synthesizeDelete("User", args);
        const envelope = await this.executor.executeUnit(
            buildDeclaredOperationRequest("User.delete", synthesis),
        );
        if (!envelope.ok) {
            throwOperationEnvelopeError(envelope);
        }
    }
}
export class PostDelegate {
    private readonly executor: OperationExecutor;
    constructor(executor: OperationExecutor) {
        this.executor = executor;
    }
    async findMany<const A extends PostFindManyArgs>(
        args?: A,
    ): Promise<ReadonlyArray<PostGetPayload<A>>> {
        const synthesis = synthesizeFindMany("Post", args);
        const envelope = await this.executor.execute(
            buildDeclaredOperationRequest("Post.findMany", synthesis),
        );
        if (!envelope.ok) {
            throwOperationEnvelopeError(envelope);
        }
        return envelope.value as ReadonlyArray<PostGetPayload<A>>;
    }
    async findFirst<const A extends PostFindFirstArgs>(
        args?: A,
    ): Promise<PostGetPayload<A> | null> {
        const synthesis = synthesizeFindFirst("Post", args);
        const envelope = await this.executor.execute(
            buildDeclaredOperationRequest("Post.findFirst", synthesis),
        );
        if (!envelope.ok) {
            throwOperationEnvelopeError(envelope);
        }
        const rows = envelope.value;
        return (rows[0] ?? null) as PostGetPayload<A> | null;
    }
    async findUnique<const A extends PostFindUniqueArgs>(
        args: A,
    ): Promise<PostGetPayload<A> | null> {
        const synthesis = synthesizeFindUnique("Post", args);
        const envelope = await this.executor.execute(
            buildDeclaredOperationRequest("Post.findUnique", synthesis),
        );
        if (!envelope.ok) {
            throwOperationEnvelopeError(envelope);
        }
        const rows = envelope.value;
        return (rows[0] ?? null) as PostGetPayload<A> | null;
    }
    async create<const A extends PostCreateArgs>(
        args: A,
    ): Promise<PostGetPayload<A>> {
        const synthesis = synthesizeCreate("Post", args);
        const envelope = await this.executor.execute(
            buildDeclaredOperationRequest("Post.create", synthesis),
        );
        if (!envelope.ok) {
            throwOperationEnvelopeError(envelope);
        }
        return envelope.value[0] as PostGetPayload<A>;
    }
    async update<const A extends PostUpdateArgs>(
        args: A,
    ): Promise<PostGetPayload<A>> {
        const synthesis = synthesizeUpdate("Post", args);
        const envelope = await this.executor.execute(
            buildDeclaredOperationRequest("Post.update", synthesis),
        );
        if (!envelope.ok) {
            throwOperationEnvelopeError(envelope);
        }
        return envelope.value[0] as PostGetPayload<A>;
    }
    async delete(args: PostDeleteArgs): Promise<void> {
        const synthesis = synthesizeDelete("Post", args);
        const envelope = await this.executor.executeUnit(
            buildDeclaredOperationRequest("Post.delete", synthesis),
        );
        if (!envelope.ok) {
            throwOperationEnvelopeError(envelope);
        }
    }
}
export class DbClient {
    readonly user: UserDelegate;
    readonly post: PostDelegate;
    readonly $macros: GeneratedMacros;
    private readonly executor: OperationExecutor;
    constructor(executor: OperationExecutor) {
        this.executor = executor;
        this.$macros = new GeneratedMacros(executor);
        this.user = new UserDelegate(executor);
        this.post = new PostDelegate(executor);
    }
    async $close(): Promise<void> {
        await this.executor.close();
    }
}
/** Construct a client from an injected operation executor (preferred ABI). */
export function createClient(executor: OperationExecutor): DbClient {
    return new DbClient(executor);
}
/** --- Projected operation stubs (from VOS OperationProjectionArtifact) ---
 * These delegate to `this.executor` with the declared operation id.
 */
export class GeneratedOperations {
    private readonly executor: OperationExecutor;
    constructor(executor: OperationExecutor) {
        this.executor = executor;
    }
    /** VOS macro operation `seed_blog` (op_id: op_83d9146745765d0b7e4ffec3106540cec2437a2597e8a8faf1d044fb54dcb81e). */
async seed_blog(): Promise<void> {
        const synthesis = synthesizeOperationCall("seed_blog", []);
        const envelope = await this.executor.executeUnit(buildDeclaredOperationRequest("op_83d9146745765d0b7e4ffec3106540cec2437a2597e8a8faf1d044fb54dcb81e", synthesis));
        if (!envelope.ok) {
            throwOperationEnvelopeError(envelope);
        }
    }
}
