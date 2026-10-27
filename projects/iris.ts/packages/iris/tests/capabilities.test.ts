import assert from "node:assert/strict";
import { test } from "node:test";

import { negotiateCapabilities } from "../src/runtime/negotiate-capabilities.ts";
import {
    defaultDurability,
    hostExecutionCapabilities,
    hostSupportsStorage,
    normalizeIrisHost,
} from "../src/types/profile.ts";

// ── Legacy host alias ──────────────────────────────────────────────────────

test("normalizeIrisHost maps legacy web to browser", () => {
    assert.equal(normalizeIrisHost("web"), "browser");
    assert.equal(normalizeIrisHost("node"), "node");
});

// ── Durability is owned by storage, not host ──────────────────────────────

test("defaultDurability follows storage profile", () => {
    assert.equal(defaultDurability("memory"), "ephemeral");
    assert.equal(defaultDurability("sqlite"), "local-durable");
    assert.equal(defaultDurability("d1"), "remote-durable");
    assert.equal(defaultDurability("yyds"), "remote-durable");
    assert.equal(defaultDurability("remote-http"), "remote-durable");
});

// ── Host × storage compatibility gate ──────────────────────────────────────

test("hostSupportsStorage enforces host × storage cartesian matrix", () => {
    // node binds everything
    assert.equal(hostSupportsStorage("node", "memory"), true);
    assert.equal(hostSupportsStorage("node", "sqlite"), true);
    assert.equal(hostSupportsStorage("node", "d1"), true);
    assert.equal(hostSupportsStorage("node", "yyds"), true);
    assert.equal(hostSupportsStorage("node", "remote-http"), true);
    // browser: memory and remote-http only
    assert.equal(hostSupportsStorage("browser", "memory"), true);
    assert.equal(hostSupportsStorage("browser", "d1"), false);
    assert.equal(hostSupportsStorage("browser", "remote-http"), true);
    // cf-worker / workerd: memory + d1 + remote-http
    assert.equal(hostSupportsStorage("cloudflare-worker", "d1"), true);
    assert.equal(hostSupportsStorage("cloudflare-worker", "sqlite"), false);
    assert.equal(hostSupportsStorage("workerd", "d1"), true);
    // wasi: memory only
    assert.equal(hostSupportsStorage("wasi", "memory"), true);
    assert.equal(hostSupportsStorage("wasi", "remote-http"), false);
});

// ── Execution surface comes from host alone ────────────────────────────────

test("hostExecutionCapabilities mirrors host enum", () => {
    assert.deepEqual(hostExecutionCapabilities("node"), { native: true, wasm: false, async: false, cancellation: false });
    assert.deepEqual(hostExecutionCapabilities("browser"), { native: false, wasm: true, async: true, cancellation: false });
    assert.deepEqual(hostExecutionCapabilities("cloudflare-worker"), { native: false, wasm: false, async: true, cancellation: false });
    assert.deepEqual(hostExecutionCapabilities("wasi"), { native: false, wasm: true, async: false, cancellation: false });
});

// ── negotiateCapabilities takes storage explicitly (no host inference) ─────

test("negotiateCapabilities passes storage through unchanged", () => {
    // Node + memory
    const nodeMemory = negotiateCapabilities({ host: "node", storage: "memory", bindingReady: true });
    assert.equal(nodeMemory.host, "node");
    assert.equal(nodeMemory.storage, "memory");
    assert.equal(nodeMemory.durability, "ephemeral");
    assert.equal(nodeMemory.execution.native, true);
    assert.equal(nodeMemory.storageBackends.sqlite, true);
    assert.equal(nodeMemory.storageBackends.yyds, true);
    assert.equal(nodeMemory.transaction.local, true);

    // Browser + memory
    const browserMemory = negotiateCapabilities({ host: "browser", storage: "memory", bindingReady: true });
    assert.equal(browserMemory.host, "browser");
    assert.equal(browserMemory.storage, "memory");
    assert.equal(browserMemory.execution.wasm, true);
    assert.equal(browserMemory.storageBackends.sqlite, false);
    assert.equal(browserMemory.storageBackends.d1, false);

    // Cloudflare-worker + d1 (was previously auto-inferred — now explicit)
    const workerD1 = negotiateCapabilities({ host: "cloudflare-worker", storage: "d1", bindingReady: false });
    assert.equal(workerD1.host, "cloudflare-worker");
    assert.equal(workerD1.storage, "d1");
    assert.equal(workerD1.durability, "remote-durable");
    assert.equal(workerD1.storageBackends.d1, true);
    assert.equal(workerD1.transaction.batch, true);

    // Cloudflare-worker + memory (was impossible under auto-inference)
    const workerMemory = negotiateCapabilities({ host: "cloudflare-worker", storage: "memory", bindingReady: false });
    assert.equal(workerMemory.storage, "memory");
    assert.equal(workerMemory.durability, "ephemeral");
    assert.equal(workerMemory.transaction.local, true);
});

test("negotiateCapabilities throws on incompatible host × storage pair", () => {
    // browser cannot bind d1
    assert.throws(
        () => negotiateCapabilities({ host: "browser", storage: "d1", bindingReady: false }),
        /does not support storage/,
    );
    // node cannot bind browser-only? wait — node binds everything. Check wasi + d1
    assert.throws(
        () => negotiateCapabilities({ host: "wasi", storage: "d1", bindingReady: false }),
        /does not support storage/,
    );
});
