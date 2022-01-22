/** Host that owns the Iris runtime binding. */
export type IrisHost = "node" | "web";

/** Capability surface negotiated for the current host. */
export interface IrisCapabilities {
    readonly host: IrisHost;
}

/** Shared runtime contract across Node N-API and browser WASM hosts. */
export interface IrisRuntime {
    readonly host: IrisHost;
    readonly capabilities: IrisCapabilities;
}
