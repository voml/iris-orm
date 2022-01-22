import type { IrisRuntime } from "../types/protocol.ts";

export type CreateIrisWebOptions = {
    /** Inline VOS / project source for browser-only hosts. */
    source?: unknown;
};

/** Create a browser Iris runtime (WASM semantic core + TS Web adapters). */
export async function createIris(_options: CreateIrisWebOptions = {}): Promise<IrisRuntime> {
    throw new Error("@yydb/iris: createIris is not implemented yet on the browser host");
}
