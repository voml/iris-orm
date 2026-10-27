import type { IrisCapabilityMatrix, IrisStorageCapabilities } from "../types/capabilities.ts";
import type { IrisHost, IrisStorageProfile } from "../types/profile.ts";
import { defaultDurability, hostSupportsStorage, hostExecutionCapabilities } from "../types/profile.ts";

export type NegotiateCapabilitiesInput = {
    host: IrisHost;
    storage: IrisStorageProfile;
    bindingReady: boolean;
};

function storageBackendForHost(host: IrisHost): IrisStorageCapabilities {
    switch (host) {
        case "node":
            return { memory: true, sqlite: true, d1: true, yyds: true, remoteHttp: true };
        case "browser":
            return { memory: true, sqlite: false, d1: false, yyds: false, remoteHttp: true };
        case "cloudflare-worker":
        case "workerd":
            return { memory: true, sqlite: false, d1: true, yyds: false, remoteHttp: true };
        case "wasi":
            return { memory: true, sqlite: false, d1: false, yyds: false, remoteHttp: false };
    }
}

function transactionForStorage(storage: IrisStorageProfile): IrisCapabilityMatrix["transaction"] {
    switch (storage) {
        case "memory":
            return { local: true, batch: false, crossRequest: false };
        case "sqlite":
            return { local: true, batch: false, crossRequest: false };
        case "d1":
            return { local: false, batch: true, crossRequest: false };
        case "yyds":
            return { local: false, batch: true, crossRequest: true };
        case "remote-http":
            return { local: false, batch: false, crossRequest: true };
    }
}

/**
 * Derive the capability matrix for a **host + storage pair**.
 *
 * P0 change: storage is **explicitly passed**. No host-to-storage auto-
 * inference happens. If `host` does not support `storage`, this function
 * throws a structured `Error` — the caller must pick a compatible pair.
 */
export function negotiateCapabilities(input: NegotiateCapabilitiesInput): IrisCapabilityMatrix {
    if (!hostSupportsStorage(input.host, input.storage)) {
        throw new Error(
            `@yydb/iris: host \`${input.host}\` does not support storage \`${input.storage}\` — pick a compatible pair`,
        );
    }
    return {
        host: input.host,
        storage: input.storage,
        durability: defaultDurability(input.storage),
        bindingReady: input.bindingReady,
        execution: hostExecutionCapabilities(input.host),
        storageBackends: storageBackendForHost(input.host),
        transaction: transactionForStorage(input.storage),
        limits: {},
    };
}
