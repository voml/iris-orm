import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { INVALID_SCHEMA, USER_SCHEMA, USER_SCHEMA_FINGERPRINT } from "./fixtures.ts";

const entry = new URL("../src/index.ts", import.meta.url).href;

test("wasm artifact lives under lib/", () => {
    readFileSync(fileURLToPath(new URL("../lib/iris.unknown-wasm32.wasm", import.meta.url)));
});

test("initWasm + checkSource validate fixture schema", async () => {
    const { initWasm, irisVersion, checkSource } = await import(entry);
    await initWasm();

    const version = irisVersion();
    assert.ok(version && version !== "0.0.0");

    const result = checkSource(USER_SCHEMA);
    assert.equal(result.ok, true);
    assert.equal(result.tableCount, 1);
    assert.equal(result.schemaFingerprint, USER_SCHEMA_FINGERPRINT);
    assert.equal(result.generatorVersion, version);
});

test("checkSource rejects invalid schema", async () => {
    const { initWasm, checkSource } = await import(entry);
    await initWasm();
    const bad = checkSource(INVALID_SCHEMA);
    assert.equal(bad.ok, false);
});

test("irisVersion throws before initWasm", async () => {
    const { irisVersion, resetWasmBindingForTests } = await import(entry);
    resetWasmBindingForTests();
    assert.throws(() => irisVersion(), /call initWasm\(\) before semantic core methods/);
});

test("introspectSchema returns table metadata", async () => {
    const { initWasm, introspectSchema } = await import(entry);
    await initWasm();
    const intro = JSON.parse(introspectSchema(USER_SCHEMA)) as { ok: boolean; tables: { name: string }[] };
    assert.equal(intro.ok, true);
    assert.equal(intro.tables[0]?.name, "User");
});

test("openMemorySession executes VOS", async () => {
    const { initWasm, openMemorySession } = await import(entry);
    await initWasm();
    const session = openMemorySession();
    const raw = session.executeVos('table User { @@id: utf8, name: utf8 } insert User { id: "1", name: "Ada" }');
    const result = JSON.parse(raw) as { ok: boolean };
    assert.equal(typeof result.ok, "boolean");
    session.close();
});

test("initWasm is idempotent", async () => {
    const { initWasm, irisVersion } = await import(entry);
    await initWasm();
    const first = irisVersion();
    await initWasm();
    assert.equal(irisVersion(), first);
});
