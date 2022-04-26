export interface WasmCheckSourceResult {
    ok: boolean;
    tableCount: number;
    schemaFingerprint: string;
    generatorVersion: string;
    error?: string;
    free(): void;
}

export interface WasmMemorySession {
    query(source: string, parametersJson?: string | null): string;
    execute(source: string, parametersJson?: string | null): string;
    executeVos(source: string, parametersJson?: string | null): string;
    close(): void;
}

export interface WasmMemorySessionConstructor {
    new (): WasmMemorySession;
}

export function checkSource(source: string): WasmCheckSourceResult;
export function irisVersion(): string;
export function introspectSchema(source: string): string;
export function executeVosMemory(source: string): string;
export const MemorySession: WasmMemorySessionConstructor;

export default function init(input: {
    module_or_path: URL | Request | Response | ArrayBuffer | Uint8Array | WebAssembly.Module;
}): Promise<void>;
