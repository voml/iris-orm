/**
 * Cloudflare D1 host bridge — TS-side glue between build-time Iris contracts
 * and the `prepare_d1_operation` / `decode_d1_result` WASM exports.
 *
 * This module is the **only** place that touches `@yydb/iris-unknown-wasm32`
 * from the Cloudflare Worker host path.  The build-time Rust adapter lowers
 * every operation into a `D1Request` envelope (SQL + positional wire params
 * + execution mode + returning projection + contract fingerprint); this
 * bridge takes that envelope through the WASM semantic core, executes it
 * against `env.DB`, and hands the canonical value rows back to the Iris
 * runtime.
 *
 * Wire shape source of truth: `iris-adapter-d1::*` (Rust).
 *
 * # Flow
 *
 * ```text
 * D1Request JSON
 *  │  prepareD1Operation  (WASM — round-trip + capability / fingerprint check)
 *  ▼
 * { sql, params: WireValue[], mode, returning, contract_fingerprint }
 *  │  wireValueToD1Bindable  →  JS bindable values
 *  │  env.DB.prepare(sql).bind(...).run() / .all()
 *  ▼
 * D1RawResult JSON (success, results, changes, last_row_id, column_names)
 *  │  decodeD1Result  (WASM — WireValue → canonical VOS Value, row canonicalisation)
 *  ▼
 * CanonicalResult { rows, row_count, last_insert_rowid }
 * ```
 */

// ---------------------------------------------------------------------------
// Wire-value types (mirror `iris_adapter_d1::WireValue` serde shape)
// ---------------------------------------------------------------------------

/**
 * D1 wire carrier for a single parameter or result column.
 *
 * Serde tag/content shape: `#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]`.
 */
export type WireValue =
    | { readonly kind: "null" }
    | { readonly kind: "bool"; readonly value: boolean }
    | { readonly kind: "int"; readonly value: number | string }
    | { readonly kind: "f64"; readonly value: number }
    | { readonly kind: "str"; readonly value: string }
    | { readonly kind: "blob"; readonly value: readonly number[] };

/** Execution mode carried by a D1Request (kebab-case, matches Rust). */
export type D1ExecutionMode = "read" | "write" | "write-returning" | "batch" | "migration";

/** `RETURNING` projection descriptor (kebab-case, matches Rust). */
export type ReturningProjection =
    | { readonly kind: "none" }
    | { readonly kind: "all" }
    | { readonly kind: "columns"; readonly columns: readonly string[] };

/**
 * Fully-lowered D1 request envelope — input to `prepareD1Operation`.
 *
 * This is the build-time artifact that Rust `iris-adapter-d1` produces
 * after VOS → D1 lowering.  The WASM `prepareD1Operation` export is a
 * pure serde round-trip plus capability / contract-fingerprint gate.
 */
export interface D1Request {
    readonly sql: string;
    readonly params: readonly WireValue[];
    readonly mode: D1ExecutionMode;
    readonly returning: ReturningProjection;
    readonly contract_fingerprint: string;
}

// ---------------------------------------------------------------------------
// D1 host surface (duck-typed — no Workers types dep)
// ---------------------------------------------------------------------------

/**
 * Minimal Cloudflare D1 `.run()` result (duck-typed).
 *
 * Fields are all optional so this surface doubles as the SQLite plan
 * executor's mock — only `success` is guaranteed.
 */
export interface D1RunResult {
    readonly success?: boolean;
    readonly changes?: number;
    readonly lastRowId?: number | null;
    readonly lastInsertRowid?: number | null;
    readonly error?: string;
}

/**
 * Minimal Cloudflare D1 `.all()` result (duck-typed).
 *
 * `results` rows come back as named or positional objects; the bridge
 * emits them as wire-value arrays after WASM canonicalisation.
 */
export interface D1AllResult<T = unknown> {
    readonly success?: boolean;
    readonly results?: readonly T[];
    readonly columns?: readonly string[];
    readonly error?: string;
}

/** Positional D1 row (unnamed value column). */
export type D1PositionalRow = readonly unknown[];

/** Named D1 row (column-name keys). */
export type D1NamedRow = Record<string, unknown>;

/**
 * Duck-typed Cloudflare D1 database surface consumed by this bridge.
 *
 * Callers pass their `env.DB` directly — no `@cloudflare/workers-types`
 * dependency is taken in this package.
 */
export interface D1HostDb {
    prepare(sql: string): {
        bind(...values: unknown[]): {
            run(): Promise<D1RunResult>;
            all<T = D1PositionalRow>(): Promise<D1AllResult<T>>;
        };
    };
}

// ---------------------------------------------------------------------------
// D1 raw-result + canonical-result envelopes (mirror WASM serde shape)
// ---------------------------------------------------------------------------

/**
 * JSON envelope we hand to `decodeD1Result`.
 *
 * Matches the Rust `D1RawResult` serde layout — note the snake_case keys
 * (`last_row_id`, `column_names`, `contract_fingerprint`) because Rust uses
 * field identifiers; the wire serde preserves them as-is.
 */
export interface D1RawResultJson {
    readonly success: boolean;
    readonly results: readonly (readonly WireValue[])[];
    readonly changes: number;
    readonly last_row_id?: number | null;
    readonly column_names?: readonly string[];
    readonly error?: string;
}

/**
 * Canonical result returned from `decodeD1Result`.
 *
 * Each row value is a tagged union mirroring the VOS `iris_types::Value`
 * serde shape (`{ "Int": 42 }`, `{ "Str": "hi" }`, `null`, etc.).  The
 * bridge leaves the final VOS-Value → JS-carrier mapping to the Iris
 * runtime host that owns the `OperationExecutor`.
 */
export interface CanonicalResult {
    readonly rows: readonly Record<string, unknown>[];
    readonly row_count: number;
    readonly last_insert_rowid?: number | null;
}

// ---------------------------------------------------------------------------
// WASM loader (duck-typed around `@yydb/iris-unknown-wasm32`)
// ---------------------------------------------------------------------------

/** Subset of the wasm package surface the bridge calls. */
interface D1WasmExports {
    /** Accepts a serialised `D1Request`, returns the same envelope JSON. */
    prepareD1Operation?(requestJson: string): string;
    /** Accepts a serialised `D1RawResult`, returns a `CanonicalResult` JSON string. */
    decodeD1Result?(rawJson: string): string;
}

let wasmApi: D1WasmExports | undefined;

/**
 * Lazy-load the WASM semantic core's D1 bridge exports.
 *
 * The underlying package is `@yydb/iris-unknown-wasm32` (workspace dep of
 * `@yydb/iris-language`).  `prepareD1Operation` / `decodeD1Result` are
 * plain synchronous functions — `wasm-bindgen` does not make them async.
 */
async function getWasmApi(): Promise<D1WasmExports> {
    if (wasmApi) {
        return wasmApi;
    }
    const pkg = (await import("@yydb/iris-unknown-wasm32")) as unknown as D1WasmExports;
    wasmApi = pkg;
    return pkg;
}

// ---------------------------------------------------------------------------
// WireValue ↔ D1 bindable conversion
// ---------------------------------------------------------------------------

/**
 * Convert a `WireValue` into a value Cloudflare D1 `.bind()` accepts.
 *
 * D1 accepts `string | number | boolean | ArrayBuffer | Uint8Array | null`.
 * Our WireValue may carry:
 *   - `int` → `number` (D1 INTEGER affinity restores precision for integer text)
 *   - `f64` → `number`
 *   - `str` / `bool` / `null` → pass-through
 *   - `blob` → `Uint8Array(value)` (D1 carries BLOB as binary)
 */
export function wireValueToD1Bindable(wv: WireValue): unknown {
    switch (wv.kind) {
        case "null":
            return null;
        case "bool":
            return wv.value;
        case "int":
            // int arrives as number (JSON-safe) or string (bigint text form).
            // D1 `.bind()` coerces both sides via SQLite affinity.
            return typeof wv.value === "number" ? wv.value : wv.value;
        case "f64":
            return wv.value;
        case "str":
            return wv.value;
        case "blob":
            return new Uint8Array(wv.value as readonly number[]);
    }
}

// ---------------------------------------------------------------------------
// WASM bridge primitives
// ---------------------------------------------------------------------------

/**
 * Call the WASM `prepareD1Operation` export.
 *
 * Returns the round-tripped envelope (identical to input when capability /
 * fingerprint checks pass; an error is thrown otherwise).
 */
async function wasmPrepare(d1Request: D1Request): Promise<D1Request> {
    const api = await getWasmApi();
    if (typeof api.prepareD1Operation !== "function") {
        throw new Error(
            "@yydb/iris-language/cloudflare: wasm package does not export prepareD1Operation — rebuild @yydb/iris-unknown-wasm32 with the D1 adapter feature",
        );
    }
    const json = JSON.stringify(d1Request);
    const out = api.prepareD1Operation(json);
    return JSON.parse(out) as D1Request;
}

/**
 * Call the WASM `decodeD1Result` export.
 *
 * Converts the D1 raw wire result into the canonical VOS-value envelope
 * the Iris runtime consumes.
 */
async function wasmDecode(raw: D1RawResultJson): Promise<CanonicalResult> {
    const api = await getWasmApi();
    if (typeof api.decodeD1Result !== "function") {
        throw new Error(
            "@yydb/iris-language/cloudflare: wasm package does not export decodeD1Result — rebuild @yydb/iris-unknown-wasm32 with the D1 adapter feature",
        );
    }
    const json = JSON.stringify(raw);
    const out = api.decodeD1Result(json);
    return JSON.parse(out) as CanonicalResult;
}

// ---------------------------------------------------------------------------
// Cloudflare D1 executor helpers
// ---------------------------------------------------------------------------

/**
 * Execute a prepared read request against a D1 database.
 *
 * We use `.all()` with an *empty* column list — D1 returns positional rows
 * when the call didn't request column names — then patch `column_names`
 * into the D1RawResult envelope so the WASM decode can still label rows
 * (falling back to `col_0`, `col_1`, … when absent).
 */
async function executeReadAll(
    db: D1HostDb,
    sql: string,
    bindValues: readonly unknown[],
): Promise<D1RawResultJson> {
    const stmt = db.prepare(sql).bind(...bindValues);
    const res = await stmt.all<D1PositionalRow>();
    // Undefined `success` (old mocks, SQLite plan executor) defaults to true.
    if (res.success === false) {
        return {
            success: false,
            results: [],
            changes: 0,
            error: res.error ?? "D1 .all() failed",
        };
    }
    const results = (res.results ?? []).map((row) =>
        (row as readonly unknown[]).map((v) => jsValueToWireValue(v)),
    );
    return {
        success: true,
        results,
        changes: 0,
        column_names: res.columns ? [...res.columns] : undefined,
    };
}

/**
 * Execute a plain write request (no `RETURNING`) via `.run()`.
 *
 * `.run()` does not return rows; we synthesise an empty-rows raw result
 * so the WASM decode stays shape-valid.
 */
async function executeWriteRun(
    db: D1HostDb,
    sql: string,
    bindValues: readonly unknown[],
): Promise<D1RawResultJson> {
    const stmt = db.prepare(sql).bind(...bindValues);
    const res = await stmt.run();
    const success = res.success !== false;
    return {
        success,
        results: [],
        changes: res.changes ?? 0,
        last_row_id: (res.lastRowId ?? res.lastInsertRowid ?? null) as number | null,
        error: success ? undefined : (res.error ?? "D1 .run() failed"),
    };
}

/**
 * JS scalar → WireValue (D1 raw rows OR runtime parameter values).
 *
 * Cloudflare D1 `.all()` returns native JS values (number | string | boolean |
 * null | ArrayBuffer).  We normalise each back into a WireValue before
 * handing to the WASM decode, so the `decode_d1_result` serde round-trip is
 * identical whether the rows came from D1 directly or were injected by tests.
 *
 * Also exposed for executor callers that need to build a D1Request envelope
 * from runtime `OperationRequest.parameters` (JS values → WireValue[]).
 */
export function jsValueToWireValue(v: unknown): WireValue {
    if (v === null || v === undefined) return { kind: "null" };
    if (typeof v === "boolean") return { kind: "bool", value: v };
    if (typeof v === "bigint") {
        const n = Number(v);
        return Number.isSafeInteger(n) ? { kind: "int", value: n } : { kind: "int", value: v.toString() };
    }
    if (typeof v === "number") {
        if (Number.isInteger(v)) return { kind: "int", value: v };
        return { kind: "f64", value: v };
    }
    if (typeof v === "string") return { kind: "str", value: v };
    if (v instanceof Uint8Array) {
        return { kind: "blob", value: Array.from(v) };
    }
    if (v instanceof ArrayBuffer) {
        return { kind: "blob", value: Array.from(new Uint8Array(v)) };
    }
    // Fall back to string — better a lost value than a shape-valid crash.
    return { kind: "str", value: String(v) };
}

// ---------------------------------------------------------------------------
// Public bridge API
// ---------------------------------------------------------------------------

/**
 * Full D1Request → CanonicalResult bridge.
 *
 * 1. `wasm.prepareD1Operation(d1RequestJson)` — serde round-trip with
 *    capability / contract-fingerprint gate.
 * 2. Execute against `env.DB` via the prepared `mode` (read→`.all()`,
 *    write→`.run()`, write-returning→`.all()`).
 * 3. `wasm.decodeD1Result(rawJson)` — WireValue → canonical VOS Value.
 */
export async function executeD1Request(
    d1Request: D1Request,
    db: D1HostDb,
): Promise<CanonicalResult> {
    const prepared = await wasmPrepare(d1Request);

    const bindValues = prepared.params.map(wireValueToD1Bindable);
    let raw: D1RawResultJson;
    switch (prepared.mode) {
        case "read":
        case "write-returning":
            raw = await executeReadAll(db, prepared.sql, bindValues);
            break;
        case "write":
            raw = await executeWriteRun(db, prepared.sql, bindValues);
            break;
        case "migration":
            // Not wired through D1.prepare() + bind() — migrations take a
            // separate `DB.exec()` path in the future.  Return success=true
            // empty result so callers get a shape-valid no-op until that
            // path lands.
            raw = { success: true, results: [], changes: 0 };
            break;
        case "batch":
            throw new Error(
                "@yydb/iris-language/cloudflare: batch D1Request not supported yet — use DB.batch() host binding",
            );
        default:
            throw new Error(
                `@yydb/iris-language/cloudflare: unknown D1ExecutionMode '${prepared.mode}'`,
            );
    }

    return wasmDecode(raw);
}

// ---------------------------------------------------------------------------
// Canonical VOS Value → JS carrier conversion
// ---------------------------------------------------------------------------

/**
 * Convert a single canonical VOS `Value` (as returned from `decodeD1Result`)
 * into a JS carrier value (number | string | boolean | null | bigint).
 *
 * The VOS `Value` serde shape is externally-tagged:
 *   - `null` → null
 *   - `{ Bool: true }` → boolean
 *   - `{ Int: 42 }` → number (safe integer) or BigInt (i64 overflow)
 *   - `{ Str: "hi" }` / `{ Decimal: "3.14" }` / `{ Date: "..." }` / … → string
 *   - `{ Object: {...} }` → nested canonical row (caller decides how to surface)
 */
export function canonicalValueToCarrier(v: unknown): unknown {
    if (v === null || v === undefined) return null;
    if (typeof v !== "object") {
        // bool / number / string already scalar (unit-variant Null from serde).
        return v;
    }
    const obj = v as Record<string, unknown>;
    const keys = Object.keys(obj);
    if (keys.length !== 1) {
        // Unexpected shape — pass through for diagnostics.
        return v;
    }
    const tag = keys[0]!;
    const inner = obj[tag];
    switch (tag) {
        case "Bool":
        case "Str":
        case "Decimal":
        case "Date":
        case "Time":
        case "DateTime":
            return inner;
        case "Int": {
            const n = inner as number;
            return Number.isSafeInteger(n) ? n : BigInt(n);
        }
        case "Object":
            // Recurse into nested row; surface as a JS object with carriers.
            return canonicalRowToCarrierRow(inner as Record<string, unknown>);
        case "Null":
            return null;
        default:
            return inner;
    }
}

/** Convert a canonical VOS Value row (from `decodeD1Result`) into plain JS carriers. */
export function canonicalRowToCarrierRow(
    row: Readonly<Record<string, unknown>>,
): Record<string, unknown> {
    const out: Record<string, unknown> = {};
    for (const [key, value] of Object.entries(row)) {
        out[key] = canonicalValueToCarrier(value);
    }
    return out;
}
