import type {
    WebIrisBindingProfile,
    WebIrisCapabilityMatrix,
    WebIrisDurability,
    WebIrisHost,
    WebIrisStorageProfile,
} from "../web-contract.ts";

export type NegotiateCapabilitiesInput = {
    host: WebIrisHost;
    profile: WebIrisBindingProfile;
    bindingReady: boolean;
};

function storageProfile(host: WebIrisHost, profile: WebIrisBindingProfile): WebIrisStorageProfile {
    if (host === "cloudflare-worker") {
        return "d1";
    }
    return profile === "memory" ? "memory" : "local-fs";
}

function defaultDurability(profile: WebIrisStorageProfile): WebIrisDurability {
    switch (profile) {
        case "memory":
            return "ephemeral";
        case "local-fs":
            return "local-durable";
        case "d1":
            return "remote-durable";
    }
}

function storageForHost(host: WebIrisHost): WebIrisCapabilityMatrix["storage"] {
    switch (host) {
        case "browser":
            return { memory: true, localFs: false, opfs: false, d1: false };
        case "cloudflare-worker":
            return { memory: true, localFs: false, opfs: false, d1: true };
    }
}

function executionForHost(host: WebIrisHost): WebIrisCapabilityMatrix["execution"] {
    switch (host) {
        case "browser":
            return { native: false, wasm: true, async: true, cancellation: false };
        case "cloudflare-worker":
            return { native: false, wasm: false, async: true, cancellation: false };
    }
}

function transactionForProfile(profile: WebIrisStorageProfile): WebIrisCapabilityMatrix["transaction"] {
    switch (profile) {
        case "memory":
        case "local-fs":
            return { local: true, batch: false, crossRequest: false };
        case "d1":
            return { local: false, batch: true, crossRequest: false };
    }
}

/** Derive the capability matrix for a Web host + binding profile pair. */
export function negotiateCapabilities(input: NegotiateCapabilitiesInput): WebIrisCapabilityMatrix {
    const profile = storageProfile(input.host, input.profile);
    return {
        host: input.host,
        profile,
        durability: defaultDurability(profile),
        bindingReady: input.bindingReady,
        execution: executionForHost(input.host),
        storage: storageForHost(input.host),
        transaction: transactionForProfile(profile),
        limits: {},
    };
}
