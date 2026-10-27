import type { IrisHost, IrisStorageProfile, IrisDurability } from "./profile.ts";

/** Execution surface negotiated for the active host. */
export interface IrisExecutionCapabilities {
    readonly native: boolean;
    readonly wasm: boolean;
    readonly async: boolean;
    readonly cancellation: boolean;
}

/**
 * Storage backends a host can bind (not the active profile — see
 * `IrisCapabilityMatrix.storage` for the actually chosen backend).
 *
 * Fields mirror `IrisStorageProfile` one-to-one so `hostSupportsStorage`
 * can answer the same question with a single boolean access.
 */
export interface IrisStorageCapabilities {
    readonly memory: boolean;
    readonly sqlite: boolean;
    readonly d1: boolean;
    readonly yyds: boolean;
    readonly remoteHttp: boolean;
}

/** Transaction semantics exposed by the active storage profile. */
export interface IrisTransactionCapabilities {
    readonly local: boolean;
    readonly batch: boolean;
    readonly crossRequest: boolean;
}

/** Host-advertised resource limits (optional until enforced). */
export interface IrisLimitsCapabilities {
    readonly maxBatchStatements?: number;
    readonly maxPayloadBytes?: number;
    readonly maxResultBytes?: number;
}

/**
 * Full capability matrix for a **host + storage pair** (P0 decoupled).
 *
 * `storage` is the **explicitly chosen** storage profile — no inference from
 * `host`. `storageBackends` is the full backend menu the host can address.
 */
export interface IrisCapabilityMatrix {
    readonly host: IrisHost;
    readonly storage: IrisStorageProfile;
    readonly durability: IrisDurability;
    /** Whether the semantic core binding finished loading. */
    readonly bindingReady: boolean;
    readonly execution: IrisExecutionCapabilities;
    readonly storageBackends: IrisStorageCapabilities;
    readonly transaction: IrisTransactionCapabilities;
    readonly limits: IrisLimitsCapabilities;
}
