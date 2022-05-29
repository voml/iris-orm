import assert from "node:assert/strict";
import { test } from "node:test";

import { negotiateCapabilities } from "../src/runtime/negotiate-capabilities.ts";
import { defaultDurability, normalizeIrisHost, resolveStorageProfile } from "../src/types/profile.ts";

test("normalizeIrisHost maps legacy web to browser", () => {
    assert.equal(normalizeIrisHost("web"), "browser");
    assert.equal(normalizeIrisHost("node"), "node");
});

test("resolveStorageProfile maps binding profiles to storage profiles", () => {
    assert.equal(resolveStorageProfile("memory"), "memory");
    assert.equal(resolveStorageProfile("sqlite"), "local-fs");
    assert.equal(resolveStorageProfile("project"), "local-fs");
});

test("defaultDurability follows storage profile", () => {
    assert.equal(defaultDurability("memory"), "ephemeral");
    assert.equal(defaultDurability("local-fs"), "local-durable");
    assert.equal(defaultDurability("d1"), "remote-durable");
});

test("negotiateCapabilities separates host and storage profile", () => {
    const nodeMemory = negotiateCapabilities({ host: "node", profile: "memory", bindingReady: true });
    assert.equal(nodeMemory.host, "node");
    assert.equal(nodeMemory.profile, "memory");
    assert.equal(nodeMemory.durability, "ephemeral");
    assert.equal(nodeMemory.execution.native, true);
    assert.equal(nodeMemory.storage.localFs, true);

    const browserMemory = negotiateCapabilities({ host: "browser", profile: "memory", bindingReady: true });
    assert.equal(browserMemory.host, "browser");
    assert.equal(browserMemory.execution.wasm, true);
    assert.equal(browserMemory.storage.opfs, true);

    const workerProject = negotiateCapabilities({ host: "cloudflare-worker", profile: "project", bindingReady: false });
    assert.equal(workerProject.host, "cloudflare-worker");
    assert.equal(workerProject.profile, "local-fs");
    assert.equal(workerProject.storage.d1, true);
    assert.equal(workerProject.transaction.batch, false);
});
