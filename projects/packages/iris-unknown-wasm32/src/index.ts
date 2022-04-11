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
    /** When omitted, loads the bundled `lib/iris_wasm_bg.wasm` asset. */
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
    query(source: string, parametersJson?: string | null): string;
    execute(source: string, parametersJson?: string | null): string;
    executeVos(source: string, parametersJson?: string | null): string;
    close(): void;
};

type GlueModule = {
    default: (input: { module_or_path: WasmInitInput | ArrayBuffer | Uint8Array }) => Promise<void>;
    checkSource?: (source: string) => GlueCheckSourceResult;
    irisVersion?: () => string;
    introspectSchema?: (source: string) => string;
    executeVosMemory?: (source: string) => string;
    MemorySession?: new () => GlueMemorySession;
};

let glue: GlueModule | null = null;
let ready = false;

function assertReady(): void {
    if (!ready || glue == null) {
        throw new Error("@yydb/iris-unknown-wasm32: call initWasm() before semantic core methods");
    }
}

function glueApi(): GlueModule {
    assertReady();
    return glue!;
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

async function loadGlueModule(): Promise<GlueModule> {
    if (glue) {
        return glue;
    }
    const mod = (await import("../lib/iris_wasm.js")) as GlueModule;
    glue = mod;
    return mod;
}

async function resolveDefaultWasmBytes(): Promise<ArrayBuffer | Uint8Array | URL> {
    const wasmUrl = new URL("../lib/iris_wasm_bg.wasm", import.meta.url);
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
    const module = await loadGlueModule();
    const moduleOrPath = await normalizeInitInput(options.module);
    await module.default({ module_or_path: moduleOrPath });
    ready = true;
}

/** Library version (matches `iris::version()` / Cargo package version). */
export function irisVersion(): string {
    const api = glueApi();
    if (!api.irisVersion) {
        throw new Error("@yydb/iris-unknown-wasm32: irisVersion export missing; rebuild wasm artifacts");
    }
    return api.irisVersion();
}

/** Parse and validate schema source (same semantics as `iris check`). */
export function checkSource(source: string): CheckSourceResult {
    const api = glueApi();
    if (!api.checkSource) {
        throw new Error("@yydb/iris-unknown-wasm32: checkSource export missing; rebuild wasm artifacts");
    }
    return toCheckResult(api.checkSource(source));
}

/** Read-only schema introspection JSON. */
export function introspectSchema(source: string): string {
    const api = glueApi();
    if (!api.introspectSchema) {
        throw new Error("@yydb/iris-unknown-wasm32: introspectSchema export missing; rebuild wasm artifacts");
    }
    return api.introspectSchema(source);
}

/** Stateless in-memory execute helper. */
export function executeVosMemory(source: string): string {
    const api = glueApi();
    if (!api.executeVosMemory) {
        throw new Error("@yydb/iris-unknown-wasm32: executeVosMemory export missing; rebuild wasm artifacts");
    }
    return api.executeVosMemory(source);
}

/** Stateful in-memory reference session. */
export function openMemorySession(): GlueMemorySession {
    const api = glueApi();
    if (!api.MemorySession) {
        throw new Error("@yydb/iris-unknown-wasm32: MemorySession export missing; rebuild wasm artifacts");
    }
    return new api.MemorySession();
}

/** @internal Reset init gate (binding tests only). */
export function resetWasmBindingForTests(): void {
    ready = false;
    glue = null;
}
