import assert from "node:assert/strict";
import { test } from "node:test";

import { negotiateCapabilities } from "../src/runtime/negotiate-capabilities.ts";
import { hostSupportsStorage } from "../src/types/profile.ts";

// ── P0 smoke: host + storage combinations (explicit, no inference) ────────

test("P0 smoke: Cloudflare Worker + D1 is an explicit compatible pair", () => {
    assert.equal(hostSupportsStorage("cloudflare-worker", "d1"), true);
    const m = negotiateCapabilities({ host: "cloudflare-worker", storage: "d1", bindingReady: false });
    assert.equal(m.host, "cloudflare-worker");
    assert.equal(m.storage, "d1");
    assert.equal(m.durability, "remote-durable");
    assert.equal(m.storageBackends.d1, true);
    assert.equal(m.transaction.batch, true);
});

test("P0 smoke: Cloudflare Worker + memory (was impossible under auto-inference)", () => {
    assert.equal(hostSupportsStorage("cloudflare-worker", "memory"), true);
    const m = negotiateCapabilities({ host: "cloudflare-worker", storage: "memory", bindingReady: true });
    assert.equal(m.storage, "memory");
    assert.equal(m.durability, "ephemeral");
    assert.equal(m.transaction.batch, false);
});

test("P0 smoke: Node + SQLite (classic local durability)", () => {
    assert.equal(hostSupportsStorage("node", "sqlite"), true);
    const m = negotiateCapabilities({ host: "node", storage: "sqlite", bindingReady: true });
    assert.equal(m.storage, "sqlite");
    assert.equal(m.durability, "local-durable");
    assert.equal(m.execution.native, true);
    assert.equal(m.storageBackends.sqlite, true);
});

test("P0 smoke: Browser + memory (classic browser-only path)", () => {
    assert.equal(hostSupportsStorage("browser", "memory"), true);
    const m = negotiateCapabilities({ host: "browser", storage: "memory", bindingReady: true });
    assert.equal(m.storage, "memory");
    assert.equal(m.durability, "ephemeral");
    assert.equal(m.execution.wasm, true);
});

test("P0 smoke: Browser + D1 is correctly rejected (incompatible)", () => {
    assert.equal(hostSupportsStorage("browser", "d1"), false);
    assert.throws(
        () => negotiateCapabilities({ host: "browser", storage: "d1", bindingReady: false }),
        /does not support storage/,
    );
});
