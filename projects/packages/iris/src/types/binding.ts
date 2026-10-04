import type { IrisCapabilityMatrix } from "./capabilities.ts";
import type { CheckSourceResult } from "./check-source.ts";
import type { SchemaIntrospection } from "./schema-introspection.ts";
import type { IrisHost } from "./profile.ts";
import type { IrisSession, OpenSessionOptions } from "./session.ts";

export type { IrisHost, IrisStorageProfile, IrisBindingProfile, IrisDurability } from "./profile.ts";
export type { IrisCapabilityMatrix } from "./capabilities.ts";

/** @deprecated Use `IrisCapabilityMatrix`. */
export type IrisCapabilities = IrisCapabilityMatrix;

/**
 * Binding bring-up / conformance host (not the application ORM surface).
 *
 * Application code should import `./src/generated/iris` (or host entry) and use `DbClient` / `Database.create`.
 * Use generated `db` from `./generated`. Binding bring-up only.
 */
export interface IrisBindingHost {
    readonly host: IrisHost;
    readonly capabilities: IrisCapabilityMatrix;
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
