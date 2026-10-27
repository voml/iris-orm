/**
 * Smoke tests for the Cloudflare D1 WASM bridge.
 *
 * We inject fake `prepareD1Operation` / `decodeD1Result` and a fake
 * `env.DB` so the bridge can be exercised end-to-end without a rebuilt
 * WASM artifact or a real D1 binding.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import {
    canonicalRowToCarrierRow,
    canonicalValueToCarrier,
    executeD1Request,
    jsValueToWireValue,
    resetWasmApiForTest,
    setWasmApiForTest,
    wireValueToD1Bindable,
    type CanonicalResult,
    type D1HostDb,
    type D1Request,
    type WireValue,
} from "../src/cloudflare/d1-wasm-bridge.ts";

// ─── Helpers ──────────────────────────────────────────────────────────────

/** Fake WASM bridge: prepareD1Operation round-trips; decodeD1Result synthesises canonical rows. */
function installFakeWasm() {
    setWasmApiForTest({
        prepareD1Operation(requestJson: string): string {
            // Round-trip exactly like the Rust export.
            return requestJson;
        },
        decodeD1Result(rawJson: string): string {
            const raw = JSON.parse(rawJson) as {
                success: boolean;
                results: WireValue[][];
                column_names?: string[];
                changes?: number;
                last_row_id?: number | null;
            };
            if (!raw.success) {
                throw new Error("fake wasm decode: success=false");
            }
            const cols = raw.column_names ?? raw.results[0]?.map((_, i) => `col_${i}`) ?? [];
            const rows = raw.results.map((row) => {
                const obj: Record<string, unknown> = {};
                row.forEach((wv, i) => {
                    const col = cols[i] ?? `col_${i}`;
                    obj[col] = wireValueToCanonicalTag(wv);
                });
                return obj;
            });
            const out: CanonicalResult = {
                rows,
                row_count: raw.results.length,
                last_insert_rowid: raw.last_row_id ?? null,
            };
            return JSON.stringify(out);
        },
    });
}

/** Convert WireValue → canonical VOS Value tagged union shape (externally-tagged serde). */
function wireValueToCanonicalTag(wv: WireValue): unknown {
    switch (wv.kind) {
        case "null":
            return null;
        case "bool":
            return { Bool: wv.value };
        case "int":
            return { Int: typeof wv.value === "number" ? wv.value : Number(wv.value) };
        case "f64":
            return { Str: wv.value.toString() }; // canonical VOS has no float carrier
        case "str":
            return { Str: wv.value };
        case "blob":
            return null; // canonical VOS has no binary carrier
    }
}

/** Mock Cloudflare D1 DB that captures prepare SQL + bind values. */
function makeMockDb(): { db: D1HostDb; lastSql: string; lastBind: unknown[] } {
    const state = { lastSql: "", lastBind: [] as unknown[] };
    const db = {
        prepare(sql: string) {
            state.lastSql = sql;
            return {
                bind(...values: unknown[]) {
                    state.lastBind = values;
                    return {
                        async all() {
                            // Cloudflare D1 returns native JS values (number, string, bool, null),
                            // not WireValue tagged unions.  The bridge's executeReadAll maps them
                            // through jsValueToWireValue before handing to the WASM decode.
                            return {
                                success: true,
                                columns: ["user_id", "user_name", "active"],
                                results: [["u-1", "Ada", true]] as unknown[],
                            };
                        },
                        async run() {
                            return { success: true, changes: 1, lastRowId: 42 };
                        },
                    };
                },
            };
        },
    };
    return {
        db: db as unknown as D1HostDb,
        get lastSql() { return state.lastSql; },
        get lastBind() { return state.lastBind; },
    };
}

// ─── Tests ────────────────────────────────────────────────────────────────

test("setup / teardown WASM mock", () => {
    installFakeWasm();
    resetWasmApiForTest();
    // If no error → teardown path works.
    assert.ok(true);
});

test("jsValueToWireValue handles every JS carrier shape", () => {
    assert.deepEqual(jsValueToWireValue(null), { kind: "null" });
    assert.deepEqual(jsValueToWireValue(undefined), { kind: "null" });
    assert.deepEqual(jsValueToWireValue(true), { kind: "bool", value: true });
    assert.deepEqual(jsValueToWireValue(42), { kind: "int", value: 42 });
    assert.deepEqual(jsValueToWireValue(3.14), { kind: "f64", value: 3.14 });
    assert.deepEqual(jsValueToWireValue("hi"), { kind: "str", value: "hi" });
    assert.deepEqual(jsValueToWireValue(100n), { kind: "int", value: 100 });
    const bytes = new Uint8Array([1, 2, 3]);
    assert.deepEqual(jsValueToWireValue(bytes), { kind: "blob", value: [1, 2, 3] });
});

test("wireValueToD1Bindable converts each WireValue kind to a D1 bindable", () => {
    assert.equal(wireValueToD1Bindable({ kind: "null" }), null);
    assert.equal(wireValueToD1Bindable({ kind: "bool", value: true }), true);
    assert.equal(wireValueToD1Bindable({ kind: "int", value: 42 }), 42);
    assert.equal(wireValueToD1Bindable({ kind: "f64", value: 3.14 }), 3.14);
    assert.equal(wireValueToD1Bindable({ kind: "str", value: "hi" }), "hi");
    const blob = wireValueToD1Bindable({ kind: "blob", value: [1, 2, 3] });
    assert.ok(blob instanceof Uint8Array);
    assert.deepEqual(Array.from(blob as Uint8Array), [1, 2, 3]);
});

test("canonicalValueToCarrier maps each VOS Value tag to a JS carrier", () => {
    assert.equal(canonicalValueToCarrier(null), null);
    assert.equal(canonicalValueToCarrier({ Bool: true }), true);
    assert.equal(canonicalValueToCarrier({ Int: 42 }), 42);
    assert.equal(canonicalValueToCarrier({ Str: "hi" }), "hi");
    assert.equal(canonicalValueToCarrier({ Decimal: "3.14" }), "3.14");
});

test("canonicalRowToCarrierRow converts a full row", () => {
    const row = canonicalRowToCarrierRow({
        user_id: { Str: "u-1" },
        active: { Bool: true },
        score: { Int: 10 },
    });
    assert.deepEqual(row, { user_id: "u-1", active: true, score: 10 });
});

test("executeD1Request read path: WASM prepare → D1 .all() → WASM decode", async () => {
    installFakeWasm();
    try {
        const mock = makeMockDb();
        const req: D1Request = {
            sql: "SELECT user_id, user_name, active FROM User WHERE active = ?",
            params: [{ kind: "bool", value: true }],
            mode: "read",
            returning: { kind: "none" },
            contract_fingerprint: "cf-test-v1",
        };
        const result = await executeD1Request(req, mock.db);

        assert.equal(mock.lastSql, "SELECT user_id, user_name, active FROM User WHERE active = ?");
        assert.deepEqual(mock.lastBind, [true]);

        assert.equal(result.row_count, 1);
        assert.equal(result.rows.length, 1);
        // WASM fake decode emits canonical VOS Value tags.
        assert.deepEqual(result.rows[0], {
            user_id: { Str: "u-1" },
            user_name: { Str: "Ada" },
            active: { Bool: true },
        });
    } finally {
        resetWasmApiForTest();
    }
});

test("executeD1Request write path: WASM prepare → D1 .run() → WASM decode (empty rows)", async () => {
    installFakeWasm();
    try {
        const mock = makeMockDb();
        const req: D1Request = {
            sql: "INSERT INTO User (user_id, user_name) VALUES (?, ?)",
            params: [
                { kind: "str", value: "u-2" },
                { kind: "str", value: "Grace" },
            ],
            mode: "write",
            returning: { kind: "none" },
            contract_fingerprint: "cf-test-v1",
        };
        const result = await executeD1Request(req, mock.db);

        assert.equal(mock.lastSql, "INSERT INTO User (user_id, user_name) VALUES (?, ?)");
        assert.deepEqual(mock.lastBind, ["u-2", "Grace"]);

        // Plain write → no rows; mock .run() returned lastRowId=42.
        assert.equal(result.row_count, 0);
        assert.equal(result.rows.length, 0);
        assert.equal(result.last_insert_rowid, 42);
    } finally {
        resetWasmApiForTest();
    }
});

test("executeD1Request write-returning path uses .all() and extracts RETURNING rows", async () => {
    installFakeWasm();
    try {
        const mock = makeMockDb();
        const req: D1Request = {
            sql: "INSERT INTO User (user_id, user_name) VALUES (?, ?) RETURNING user_id, user_name, active",
            params: [
                { kind: "str", value: "u-3" },
                { kind: "str", value: "Ada" },
            ],
            mode: "write-returning",
            returning: { kind: "columns", columns: ["user_id", "user_name", "active"] },
            contract_fingerprint: "cf-test-v1",
        };
        const result = await executeD1Request(req, mock.db);

        // write-returning → .all() path (verify through the mock which emits rows).
        assert.equal(mock.lastSql.includes("RETURNING"), true);
        assert.equal(result.row_count, 1);
        assert.deepEqual(result.rows[0], {
            user_id: { Str: "u-1" },
            user_name: { Str: "Ada" },
            active: { Bool: true },
        });
    } finally {
        resetWasmApiForTest();
    }
});

test("executeD1Request surfaces WASM prepare errors", async () => {
    setWasmApiForTest({
        prepareD1Operation(_requestJson: string): string {
            throw new Error("Rust WASM: fingerprint mismatch");
        },
        decodeD1Result(_rawJson: string): string {
            return '{"rows":[],"row_count":0}';
        },
    });
    try {
        const mock = makeMockDb();
        const req: D1Request = {
            sql: "SELECT 1",
            params: [],
            mode: "read",
            returning: { kind: "none" },
            contract_fingerprint: "cf-test-v1",
        };
        await assert.rejects(
            () => executeD1Request(req, mock.db),
            /Rust WASM: fingerprint mismatch/,
        );
    } finally {
        resetWasmApiForTest();
    }
});

test("executeD1Request surfaces WASM decode errors", async () => {
    setWasmApiForTest({
        prepareD1Operation(requestJson: string): string {
            return requestJson;
        },
        decodeD1Result(_rawJson: string): string {
            throw new Error("Rust WASM: D1 decode failed");
        },
    });
    try {
        const mock = makeMockDb();
        const req: D1Request = {
            sql: "SELECT 1",
            params: [],
            mode: "read",
            returning: { kind: "none" },
            contract_fingerprint: "cf-test-v1",
        };
        await assert.rejects(
            () => executeD1Request(req, mock.db),
            /Rust WASM: D1 decode failed/,
        );
    } finally {
        resetWasmApiForTest();
    }
});
