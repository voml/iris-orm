/**
 * Browser WASM loader — consumes `@yydb/iris-unknown-wasm32` internally.
 * No public `@yydb/iris/wasm` subpath.
 */

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

/** One-time WASM init before `createIris` on browser hosts. */
export async function initIris(_options: InitIrisOptions = {}): Promise<void> {
    throw new Error(
        "@yydb/iris: browser WASM binding not implemented yet (expected @yydb/iris-unknown-wasm32)",
    );
}
