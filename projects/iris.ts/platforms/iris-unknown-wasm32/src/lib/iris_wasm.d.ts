/* tslint:disable */
/* eslint-disable */

/**
 * Result of validating a VOS / `.iris` schema source via the Rust core.
 */
export class CheckSourceResult {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    readonly error: string | undefined;
    readonly generatorVersion: string;
    readonly ok: boolean;
    readonly schemaFingerprint: string;
    readonly tableCount: number;
}

/**
 * Stateful in-memory reference session.
 */
export class MemorySession {
    free(): void;
    [Symbol.dispose](): void;
    close(): void;
    /**
     * Legacy alias of [`MemorySession::query`].
     */
    executeVos(source: string, parameters_json?: string | null): string;
    /**
     * Unit-valued / DDL-shaped VOS. Aligns with `db.$execute`.
     */
    execute(source: string, parameters_json?: string | null): string;
    constructor();
    /**
     * VOS DML (returns row JSON). Aligns with `db.$query`.
     */
    query(source: string, parameters_json?: string | null): string;
}

/**
 * Validate pre-resolved VOS artifacts (Route B counterpart of [`check_source`]).
 */
export function checkArtifacts(contract_json: string, operations_json: string): CheckSourceResult;

/**
 * Parse and validate schema source (same semantics as `iris check`).
 */
export function checkSource(source: string): CheckSourceResult;

/**
 * Accept a Cloudflare D1 raw `.all()` / `.run()` response JSON and
 * emit the canonical value envelope that the Iris runtime consumes.
 *
 * Input shape (matches D1 HTTP binding + optional column names):
 * ```json
 * {
 *   "success": true,
 *   "results": [[{ "kind": "int", "value": 1 }, { "kind": "str", "value": "alice" }]],
 *   "changes": 1,
 *   "last_row_id": 42,
 *   "column_names": ["id", "name"]
 * }
 * ```
 *
 * Output shape (canonical value rows):
 * ```json
 * {
 *   "rows": [{ "id": { "Int": 1 }, "name": { "Str": "alice" } }],
 *   "row_count": 1,
 *   "last_insert_rowid": 42
 * }
 * ```
 *
 * When `column_names` is absent rows fall back to positional keys
 * (`col_0`, `col_1`, ...) so the output is always shape-valid.
 */
export function decodeD1Result(raw_json: string): string;

/**
 * Legacy alias of [`query_memory_js`].
 */
export function executeVosMemory(source: string): string;

/**
 * Generate bindings from pre-resolved artifacts; returns JSON
 * `{ ok, schemaFingerprint, target, files: [{ name, content }], error }`.
 */
export function generateFromArtifacts(contract_json: string, operations_json: string, target: string): string;

/**
 * Read-only schema introspection JSON from pre-resolved artifacts (Route B).
 */
export function introspectFromArtifacts(contract_json: string, operations_json: string): string;

/**
 * Read-only schema introspection JSON (`GenerationModel` shape).
 */
export function introspectSchema(source: string): string;

/**
 * Library version (matches workspace `@yydb/iris` semver).
 */
export function irisVersion(): string;

/**
 * Accept a serialised [`D1Request`] and emit the lightweight prepared
 * op envelope that the TS host bridge hands to `env.DB.prepare()`.
 *
 * The envelope shape:
 * ```json
 * {
 *   "sql": "SELECT * FROM users WHERE id = ?",
 *   "params": [{ "kind": "int", "value": 1 }, ...],
 *   "mode": "read",
 *   "returning": "none"
 * }
 * ```
 *
 * This is deliberately a pure serde round-trip -- the heavy lifting
 * (SQL lowering, capability checks) happens on the Rust build side
 * before [`D1Request`] ever reaches this function.
 */
export function prepareD1Operation(request_json: string): string;

/**
 * Execute VOS DML on a fresh in-memory reference adapter; returns JSON `{ ok, rows, error }`.
 */
export function queryMemory(source: string): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_checksourceresult_free: (a: number, b: number) => void;
    readonly __wbg_memorysession_free: (a: number, b: number) => void;
    readonly checkArtifacts: (a: number, b: number, c: number, d: number) => number;
    readonly checkSource: (a: number, b: number) => number;
    readonly checksourceresult_error: (a: number) => [number, number];
    readonly checksourceresult_generatorVersion: (a: number) => [number, number];
    readonly checksourceresult_ok: (a: number) => number;
    readonly checksourceresult_schemaFingerprint: (a: number) => [number, number];
    readonly checksourceresult_tableCount: (a: number) => number;
    readonly decodeD1Result: (a: number, b: number) => [number, number, number, number];
    readonly executeVosMemory: (a: number, b: number) => [number, number];
    readonly generateFromArtifacts: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number];
    readonly introspectFromArtifacts: (a: number, b: number, c: number, d: number) => [number, number];
    readonly introspectSchema: (a: number, b: number) => [number, number];
    readonly irisVersion: () => [number, number];
    readonly memorysession_close: (a: number) => void;
    readonly memorysession_execute: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
    readonly memorysession_executeVos: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
    readonly memorysession_new: () => number;
    readonly memorysession_query: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
    readonly prepareD1Operation: (a: number, b: number) => [number, number, number, number];
    readonly queryMemory: (a: number, b: number) => [number, number];
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
