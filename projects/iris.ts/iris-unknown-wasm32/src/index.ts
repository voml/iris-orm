import initGlue, * as glue from "../iris.unknown-wasm32.js";

/** Result of validating a VOS / `.iris` schema source via the Rust core. */
export interface CheckSourceResult {
    ok: boolean;
    tableCount: number;
    schemaFingerprint: string;
    generatorVersion: string;
    error?: string | null;
}

/** WASM bytes or module handle for custom asset pipelines. */
export type WasmInitInput = URL | Request | Response | ArrayBuffer | Uint8Array | WebAssembly.Module;

export type InitWasmOptions = {
    /** When omitted, loads the bundled `lib/iris.unknown-wasm32.wasm` asset. */
    module?: WasmInitInput;
};

type GlueCheckSourceResult = {
    ok: boolean;
    tableCount: number;
    schemaFingerprint: string;
    generatorVersion: string;
    error?: string;
    free(): void;
};

type GlueMemorySession = {
    executeVos(source: string): string;
    close(): void;
};

const glueApi = glue as {
    checkSource?: (source: string) => GlueCheckSourceResult;
    irisVersion?: () => string;
    introspectSchema?: (source: string) => string;
    executeVosMemory?: (source: string) => string;
    MemorySession?: new () => GlueMemorySession;
};

let ready = false;

function assertReady(): void {
    if (!ready) {
        throw new Error("@yydb/iris-unknown-wasm32: call initWasm() before semantic core methods");
    }
}

function toCheckResult(raw: GlueCheckSourceResult): CheckSourceResult {
    const result: CheckSourceResult = {
        ok: raw.ok,
        tableCount: raw.tableCount,
        schemaFingerprint: raw.schemaFingerprint,
        generatorVersion: raw.generatorVersion,
        error: raw.error ?? null,
    };
    raw.free();
    return result;
}

async function resolveDefaultWasmBytes(): Promise<ArrayBuffer | Uint8Array | URL> {
    const wasmUrl = new URL("../lib/iris.unknown-wasm32.wasm", import.meta.url);
    if (typeof process !== "undefined" && process.versions?.node) {
        const { readFileSync } = await import("node:fs");
        const { fileURLToPath } = await import("node:url");
        return readFileSync(fileURLToPath(wasmUrl));
    }
    return wasmUrl;
}

async function normalizeInitInput(input: WasmInitInput | undefined): Promise<WasmInitInput | ArrayBuffer | Uint8Array> {
    if (input === undefined) {
        return resolveDefaultWasmBytes();
    }
    if (
        input instanceof URL ||
        input instanceof Request ||
        input instanceof Response ||
        input instanceof ArrayBuffer ||
        input instanceof Uint8Array ||
        input instanceof WebAssembly.Module
    ) {
        return input;
    }
    throw new Error(`@yydb/iris-unknown-wasm32: unsupported WASM init input (${typeof input})`);
}

/** One-time WASM init. Required before semantic core methods. */
export async function initWasm(options: InitWasmOptions = {}): Promise<void> {
    if (ready) {
        return;
    }
    const moduleOrPath = await normalizeInitInput(options.module);
    await initGlue({ module_or_path: moduleOrPath });
    ready = true;
}

/** Library version (matches `iris::version()` / Cargo package version). */
export function irisVersion(): string {
    assertReady();
    if (!glueApi.irisVersion) {
        throw new Error("@yydb/iris-unknown-wasm32: irisVersion export missing; rebuild wasm artifacts");
    }
    return glueApi.irisVersion();
}

/** Parse and validate schema source (same semantics as `iris-tools check`). */
export function checkSource(source: string): CheckSourceResult {
    assertReady();
    if (!glueApi.checkSource) {
        throw new Error("@yydb/iris-unknown-wasm32: checkSource export missing; rebuild wasm artifacts");
    }
    return toCheckResult(glueApi.checkSource(source));
}

/** Read-only schema introspection JSON. */
export function introspectSchema(source: string): string {
    assertReady();
    if (!glueApi.introspectSchema) {
        throw new Error("@yydb/iris-unknown-wasm32: introspectSchema export missing; rebuild wasm artifacts");
    }
    return glueApi.introspectSchema(source);
}

/** Stateless in-memory execute helper. */
export function executeVosMemory(source: string): string {
    assertReady();
    if (!glueApi.executeVosMemory) {
        throw new Error("@yydb/iris-unknown-wasm32: executeVosMemory export missing; rebuild wasm artifacts");
    }
    return glueApi.executeVosMemory(source);
}

/** Stateful in-memory reference session. */
export function openMemorySession(): GlueMemorySession {
    assertReady();
    if (!glueApi.MemorySession) {
        throw new Error("@yydb/iris-unknown-wasm32: MemorySession export missing; rebuild wasm artifacts");
    }
    return new glueApi.MemorySession();
}

/** @internal Reset init gate (binding tests only). */
export function resetWasmBindingForTests(): void {
    ready = false;
}
