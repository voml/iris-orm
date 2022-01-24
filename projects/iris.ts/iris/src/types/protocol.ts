/** Host that owns the Iris runtime binding. */
export type IrisHost = "node" | "web";

/** Capability surface negotiated for the current host. */
export interface IrisCapabilities {
    readonly host: IrisHost;
    /** Whether the semantic core binding is loaded (N-API or WASM). */
    readonly bindingReady: boolean;
}

/** Shared runtime contract across Node N-API and browser WASM hosts. */
export interface IrisRuntime {
    readonly host: IrisHost;
    readonly capabilities: IrisCapabilities;
}
