import type { IrisRuntime } from "../types/protocol.ts";
import { loadNativeBinding } from "./native.ts";

export type CreateIrisNodeOptions = {
    /** Path to project root or `iris.von` (default: cwd). */
    project?: string;
};

/** Create a Node Iris runtime (N-API semantic core). */
export async function createIris(_options: CreateIrisNodeOptions = {}): Promise<IrisRuntime> {
    const binding = await loadNativeBinding();
    return {
        host: "node",
        capabilities: {
            host: "node",
            bindingReady: Boolean(binding),
        },
    };
}
