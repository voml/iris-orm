/**
 * Host + storage profile definitions (P0 decoupled).
 *
 * Host and storage are fully orthogonal. Callers must **explicitly** pick a
 * storage profile together with a host. No host-to-storage auto-inference
 * is performed — the old `resolveStorageProfileForHost` path is removed.
 *
 * Capability negotiation flows through `hostSupportsStorage` (compatibility
 * check) plus the dedicated capability-accessors in `negotiate-capabilities.ts`.
 */

/** Runtime that owns the Iris binding (not storage). */
export type IrisHost = "node" | "browser" | "cloudflare-worker" | "workerd" | "wasi";

/**
 * Storage / durability profile (orthogonal to `IrisHost`).
 *
 * Each storage has well-defined capability semantics (transaction, durability,
 * batch support, cross-request scope). Host × storage combinations must pass
 * `hostSupportsStorage` before a binding can be opened.
 */
export type IrisStorageProfile = "memory" | "sqlite" | "d1" | "yyds" | "remote-http";

/** Durability class negotiated for the active storage profile. */
export type IrisDurability = "ephemeral" | "local-durable" | "remote-durable";

/** @deprecated Use `"browser"`. Kept for internal call sites migrating off `"web"`. */
export type IrisLegacyHost = "web";

/** Normalize legacy `"web"` host labels to `"browser"`. */
export function normalizeIrisHost(host: IrisHost | IrisLegacyHost): IrisHost {
    return host === "web" ? "browser" : host;
}

/** Default durability for a storage profile. */
export function defaultDurability(profile: IrisStorageProfile): IrisDurability {
    switch (profile) {
        case "memory":
            return "ephemeral";
        case "sqlite":
            return "local-durable";
        case "d1":
        case "yyds":
        case "remote-http":
            return "remote-durable";
    }
}

/**
 * Host → storage compatibility matrix.
 *
 * Returns `true` when the given host can actually bind to the given storage.
 * This is a **hard gate**: if `hostSupportsStorage` returns `false` then the
 * caller must refuse to open a binding with that pair — it is a static
 * capability gap, not a runtime error.
 *
 * ```
 *              memory  sqlite  d1    yyds  remote-http
 * node         ✅      ✅      ✅    ✅    ✅
 * browser      ✅      ❌      ❌    ❌    ✅
 * cf-worker    ✅      ❌      ✅    ❌    ✅
 * workerd      ✅      ❌      ✅    ❌    ✅
 * wasi         ✅      ❌      ❌    ❌    ❌
 * ```
 */
export function hostSupportsStorage(host: IrisHost, storage: IrisStorageProfile): boolean {
    switch (host) {
        case "node":
            return true; // node can bind every storage
        case "browser":
            return storage === "memory" || storage === "remote-http";
        case "cloudflare-worker":
        case "workerd":
            return storage === "memory" || storage === "d1" || storage === "remote-http";
        case "wasi":
            return storage === "memory";
    }
}

/**
 * Minimal host capability surface (execution only — storage support lives
 * in `hostSupportsStorage` above and the full matrix lives in
 * `IrisCapabilityMatrix`).
 *
 * This is intentionally narrow: the heavy execution/storage/transaction
 * breakdown is owned by `src/runtime/negotiate-capabilities.ts`.
 */
export interface IrisHostExecutionCapabilities {
    readonly native: boolean;
    readonly wasm: boolean;
    readonly async: boolean;
    readonly cancellation: boolean;
}

/** Return the execution capability surface advertised by a host. */
export function hostExecutionCapabilities(host: IrisHost): IrisHostExecutionCapabilities {
    switch (host) {
        case "node":
            return { native: true, wasm: false, async: false, cancellation: false };
        case "browser":
            return { native: false, wasm: true, async: true, cancellation: false };
        case "cloudflare-worker":
        case "workerd":
            return { native: false, wasm: false, async: true, cancellation: false };
        case "wasi":
            return { native: false, wasm: true, async: false, cancellation: false };
    }
}
