import { IrisFacadeError } from "../types/errors.ts";
import type { IrisRuntime } from "../types/protocol.ts";
import { assertWasmInitialized } from "./wasm.ts";

export type CreateIrisWebOptions = {
    /** Inline VOS / project source for browser-only hosts. */
    source?: unknown;
};

/** Create a browser Iris runtime (WASM semantic core). Requires `initIris()` first. */
export async function createIris(_options: CreateIrisWebOptions = {}): Promise<IrisRuntime> {
    assertWasmInitialized();
    throw new IrisFacadeError(
        "wasm-not-implemented",
        "@yydb/iris: createIris is not implemented yet on the browser host",
    );
}
