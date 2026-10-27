import assert from "node:assert/strict";
import { test } from "node:test";

import {
    bindD1Parameters,
    coerceD1BindValue,
    createD1ReadOperationExecutor,
    createIrisOperationExecutor,
    resolveD1Plan,
} from "../src/cloudflare/index.ts";
import { buildOperationRequest, declaredVosOperation } from "../src/runtime/build-operation-request.ts";
import { negotiateCapabilities } from "../src/runtime/negotiate-capabilities.ts";
import { hostSupportsStorage } from "../src/types/profile.ts";

const fakeD1 = {
    lastSql: "",
    lastBind: [] as unknown[],
    prepare(query: string) {
        this.lastSql = query;
        return {
            bind(...values: unknown[]) {
                fakeD1.lastBind = values;
                return {
                    async all<T>() {
                        return {
                            results: [{ user_id: "u1", user_name: "Ada", active: 1 }] as T[],
                        };
                    },
                    async run() {
                        return { success: true };
                    },
                };
            },
        };
    },
};

const wireNames = {
    User: {
        userId: "user_id",
        userName: "user_name",
        active: "active",
    },
};

const plans = {
    "User.findMany": { sql: "SELECT user_id, user_name, active FROM User", mode: "read" as const },
    "User.findMany@p_active": {
        sql: "SELECT user_id, user_name, active FROM User WHERE active = ?",
        mode: "read" as const,
        paramOrder: ["p_active"],
    },
    "User.findMany@p_user_name": {
        sql: "SELECT user_id, user_name, active FROM User WHERE user_name = ?",
        mode: "read" as const,
        paramOrder: ["p_user_name"],
    },
    "User.findMany@take": {
        sql: "SELECT user_id, user_name, active FROM User LIMIT ?",
        mode: "read" as const,
        paramOrder: ["take"],
    },
    "User.findUnique@p_user_id": {
        sql: "SELECT user_id, user_name, active FROM User WHERE user_id = ? LIMIT 1",
        mode: "read" as const,
        paramOrder: ["p_user_id"],
    },
    "User.create@data_active,data_user_id,data_user_name": {
        sql: "INSERT INTO User (user_id, user_name, active) VALUES (?, ?, ?) RETURNING user_id, user_name, active",
        mode: "write-returning" as const,
        paramOrder: ["data_user_id", "data_user_name", "data_active"],
    },
    "User.delete@p_user_id": {
        sql: "DELETE FROM User WHERE user_id = ?",
        mode: "write" as const,
        paramOrder: ["p_user_id"],
    },
    "User.update@p_user_id,patch_user_name": {
        sql: "UPDATE User SET user_name = ? WHERE user_id = ? RETURNING user_id, user_name, active",
        mode: "write-returning" as const,
        paramOrder: ["patch_user_name", "p_user_id"],
    },
};

// ── Pure plan utilities still work ────────────────────────────────────────

test("resolveD1Plan selects parameter variant keys", () => {
    const plan = resolveD1Plan(plans, "User.findMany", { p_active: true });
    assert.equal(plan?.sql, plans["User.findMany@p_active"].sql);
    assert.deepEqual(bindD1Parameters(plan!, { p_active: true }), [1]);
    assert.equal(coerceD1BindValue(false), 0);
});

// ── P0: D1 executor route is deprecated (runtime throws) ───────────────────

test("P0: createD1ReadOperationExecutor throws structured deprecation error", () => {
    assert.throws(
        () => createD1ReadOperationExecutor(fakeD1, plans, wireNames),
        /deprecated.*P0/,
    );
});

test("P0: createIrisOperationExecutor throws structured deprecation error", async () => {
    await assert.rejects(
        () => createIrisOperationExecutor({ d1: fakeD1, plans, wireNamesByEntity: wireNames }),
        /deprecated.*P0/,
    );
});

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
