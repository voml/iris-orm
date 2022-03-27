import type { CheckSourceResult } from "./check-source.ts";
import type { SchemaIntrospection } from "./schema-introspection.ts";
import type { IrisSession, OpenSessionOptions } from "./session.ts";

/** Host that owns the Iris runtime binding. */
export type IrisHost = "node" | "web";

/** Capability surface negotiated for the current host. */
export interface IrisCapabilities {
    readonly host: IrisHost;
    /** Whether the semantic core binding is loaded (N-API or WASM). */
    readonly bindingReady: boolean;
}

/**
 * Binding bring-up / conformance host (not the application ORM surface).
 *
 * Application code should import `./generated/iris/typescript` (or host entry) and use `DbClient` / `createDb`.
 * Use generated `db` from `./generated`. Binding bring-up only.
 */
export interface IrisBindingHost {
    readonly host: IrisHost;
    readonly capabilities: IrisCapabilities;
    version(): string;
    /** Tooling: validate schema source (CLI / agents). */
    checkSource(source: string): CheckSourceResult;
    /** Tooling: introspect GenerationModel JSON (codegen / drift). */
    introspectSchema(source: string): SchemaIntrospection;
    /**
     * Debug / VOS console session. Not the generated client query API.
     * @deprecated Prefer generated `db` + `db.$execute`.
     */
    openSession(options?: OpenSessionOptions): IrisSession;
}

/** @deprecated Use `IrisBindingHost` for binding; generated client is the app surface. */
export type IrisRuntime = IrisBindingHost;
