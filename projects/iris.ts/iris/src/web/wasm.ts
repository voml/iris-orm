import { IrisFacadeError } from "../types/errors.ts";

const WASM_PACKAGE = "@yydb/iris-unknown-wasm32";

/** Explicit WASM bytes or module handle for hosts with custom asset pipelines. */
export type WasmSource =
    | URL
    | Request
    | Response
    | ArrayBuffer
    | Uint8Array
    | WebAssembly.Module;

export type InitIrisOptions = {
    /** When omitted, the default optional platform package asset is used. */
    module?: WasmSource;
};

let initDone = false;

/** One-time WASM init before `createIris` on browser hosts. */
export async function initIris(_options: InitIrisOptions = {}): Promise<void> {
    if (initDone) {
        return;
    }
    throw new IrisFacadeError(
        "wasm-not-implemented",
        `@yydb/iris: browser WASM loader not implemented yet (optional ${WASM_PACKAGE})`,
    );
}

/** @internal */
export function assertWasmInitialized(): void {
    if (!initDone) {
        throw new IrisFacadeError(
            "wasm-not-initialized",
            "@yydb/iris: call initIris() before createIris()",
        );
    }
}
