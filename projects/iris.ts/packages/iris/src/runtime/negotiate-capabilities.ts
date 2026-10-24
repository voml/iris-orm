import type { IrisCapabilityMatrix } from "../types/capabilities.ts";
import type { IrisBindingProfile, IrisHost, IrisStorageProfile } from "../types/profile.ts";
import { defaultDurability, resolveStorageProfileForHost } from "../types/profile.ts";

export type NegotiateCapabilitiesInput = {
    host: IrisHost;
    profile: IrisBindingProfile;
    bindingReady: boolean;
};

function storageForHost(host: IrisHost): IrisCapabilityMatrix["storage"] {
    switch (host) {
        case "node":
            return { memory: true, localFs: true, opfs: false, d1: false };
        case "browser":
            return { memory: true, localFs: false, opfs: false, d1: false };
        case "cloudflare-worker":
            return { memory: true, localFs: false, opfs: false, d1: true };
    }
}

function executionForHost(host: IrisHost): IrisCapabilityMatrix["execution"] {
    switch (host) {
        case "node":
            return { native: true, wasm: false, async: false, cancellation: false };
        case "browser":
            return { native: false, wasm: true, async: true, cancellation: false };
        case "cloudflare-worker":
            return { native: false, wasm: false, async: true, cancellation: false };
    }
}

function transactionForProfile(profile: IrisStorageProfile): IrisCapabilityMatrix["transaction"] {
    switch (profile) {
        case "memory":
            return { local: true, batch: false, crossRequest: false };
        case "local-fs":
            return { local: true, batch: false, crossRequest: false };
        case "d1":
            return { local: false, batch: true, crossRequest: false };
    }
}

/** Derive the capability matrix for a host + binding profile pair. */
export function negotiateCapabilities(input: NegotiateCapabilitiesInput): IrisCapabilityMatrix {
    const storageProfile = resolveStorageProfileForHost(input.host, input.profile);
    return {
        host: input.host,
        profile: storageProfile,
        durability: defaultDurability(storageProfile),
        bindingReady: input.bindingReady,
        execution: executionForHost(input.host),
        storage: storageForHost(input.host),
        transaction: transactionForProfile(storageProfile),
        limits: {},
    };
}
