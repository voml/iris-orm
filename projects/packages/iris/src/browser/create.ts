import type { IrisBindingHost } from "../types/binding.ts";
import { getWasmSemanticCore } from "./wasm.ts";
import { buildRuntime } from "../runtime/build-runtime.ts";

export type CreateIrisBrowserOptions = {
    /** Inline VOS / project source for browser-only hosts. */
    source?: unknown;
};

/**
 * @deprecated Use generated `IrisClient` + browser executor wiring. WASM init only.
 */
export async function createIris(_options: CreateIrisBrowserOptions = {}): Promise<IrisBindingHost> {
    return buildRuntime("web", getWasmSemanticCore());
}
