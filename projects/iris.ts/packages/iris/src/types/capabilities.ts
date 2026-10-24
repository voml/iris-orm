import type { IrisDurability, IrisHost, IrisStorageProfile } from "./profile.ts";

/** Execution surface negotiated for the active host. */
export interface IrisExecutionCapabilities {
    readonly native: boolean;
    readonly wasm: boolean;
    readonly async: boolean;
    readonly cancellation: boolean;
}

/**
 * Storage backends Iris binds directly (not necessarily the active profile).
 *
 * Browser OPFS persistence is a YYDB host adapter concern. Iris must not wire
 * OPFS or other web database APIs; consumers use YYDB when persistence is needed.
 */
export interface IrisStorageCapabilities {
    readonly memory: boolean;
    readonly localFs: boolean;
    /** Always false: Iris does not implement OPFS; see YYDB Living `06` / `08`. */
    readonly opfs: boolean;
    readonly d1: boolean;
}

/** Transaction semantics exposed by the active profile. */
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
 * Full capability matrix for the active host + profile pair.
 *
 * Replaces the legacy `{ host, bindingReady }` placeholder.
 */
export interface IrisCapabilityMatrix {
    readonly host: IrisHost;
    readonly profile: IrisStorageProfile;
    readonly durability: IrisDurability;
    /** Whether the semantic core binding finished loading. */
    readonly bindingReady: boolean;
    readonly execution: IrisExecutionCapabilities;
    readonly storage: IrisStorageCapabilities;
    readonly transaction: IrisTransactionCapabilities;
    readonly limits: IrisLimitsCapabilities;
}
