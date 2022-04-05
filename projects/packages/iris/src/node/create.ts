import type { IrisBindingHost } from "../types/binding.ts";
import { buildRuntime } from "../runtime/build-runtime.ts";
import { loadIrisNative } from "./load.ts";

export type CreateIrisNodeOptions = {
    /** Path to project root or `iris.von` (default: cwd). */
    project?: string;
};

/**
 * @deprecated Use generated `IrisClient` + `createIrisExecutor()`. Binding bring-up only.
 */
export async function createIris(_options: CreateIrisNodeOptions = {}): Promise<IrisBindingHost> {
    const core = loadIrisNative();
    return buildRuntime("node", core);
}
