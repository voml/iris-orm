/** Runtime that owns the Iris binding (not storage). */
export type IrisHost = "node" | "browser" | "cloudflare-worker";

/** Storage / durability profile (orthogonal to `IrisHost`). */
export type IrisStorageProfile = "memory" | "local-fs" | "opfs" | "d1";

/** Binding options profile before host negotiation. */
export type IrisBindingProfile = "memory" | "sqlite" | "project";

/** Durability class negotiated for the active profile. */
export type IrisDurability = "ephemeral" | "local-durable" | "remote-durable";

/** @deprecated Use `"browser"`. Kept for internal call sites migrating off `"web"`. */
export type IrisLegacyHost = "web";

/** Normalize legacy `"web"` host labels to `"browser"`. */
export function normalizeIrisHost(host: IrisHost | IrisLegacyHost): IrisHost {
    return host === "web" ? "browser" : host;
}

/** Map binding `profile` options to the storage profile contract. */
export function resolveStorageProfile(profile: IrisBindingProfile): IrisStorageProfile {
    switch (profile) {
        case "memory":
            return "memory";
        case "sqlite":
        case "project":
            return "local-fs";
    }
}

/** Default durability for a storage profile. */
export function defaultDurability(profile: IrisStorageProfile): IrisDurability {
    switch (profile) {
        case "memory":
            return "ephemeral";
        case "local-fs":
        case "opfs":
            return "local-durable";
        case "d1":
            return "remote-durable";
    }
}
