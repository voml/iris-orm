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
    "User.findMany@p_userName": {
        sql: "SELECT user_id, user_name, active FROM User WHERE user_name = ?",
        mode: "read" as const,
        paramOrder: ["p_userName"],
    },
};

test("resolveD1Plan selects parameter variant keys", () => {
    const plan = resolveD1Plan(plans, "User.findMany", { p_active: true });
    assert.equal(plan?.sql, plans["User.findMany@p_active"].sql);
    assert.deepEqual(bindD1Parameters(plan!, { p_active: true }), [1]);
    assert.equal(coerceD1BindValue(false), 0);
});

test("D1 read executor binds string eq filter parameters", async () => {
    const executor = createD1ReadOperationExecutor(fakeD1, plans, wireNames);
    const read = await executor.execute(
        buildOperationRequest(
            { operationId: "User.findMany", contractFingerprint: "fp" },
            declaredVosOperation('User.filter(x => x.user_name == $p_userName).collect()'),
            { p_userName: "Ada" },
        ),
    );
    assert.equal(read.ok, true);
    assert.match(fakeD1.lastSql, /WHERE user_name = \?/);
    assert.deepEqual(fakeD1.lastBind, ["Ada"]);
});

test("D1 read executor binds bool filter parameters", async () => {
    const executor = createD1ReadOperationExecutor(fakeD1, plans, wireNames);
    const read = await executor.execute(
        buildOperationRequest(
            { operationId: "User.findMany", contractFingerprint: "fp" },
            declaredVosOperation("User.filter(x => x.active == $p_active).collect()"),
            { p_active: true },
        ),
    );
    assert.equal(read.ok, true);
    assert.match(fakeD1.lastSql, /WHERE active = \?/);
    assert.deepEqual(fakeD1.lastBind, [1]);
});

test("negotiateCapabilities maps cloudflare-worker host to d1 profile", () => {
    const matrix = negotiateCapabilities({ host: "cloudflare-worker", profile: "d1", bindingReady: false });
    assert.equal(matrix.host, "cloudflare-worker");
    assert.equal(matrix.profile, "d1");
    assert.equal(matrix.storage.d1, true);
});

test("createIrisOperationExecutor requires d1 binding", async () => {
    await assert.rejects(
        () => createIrisOperationExecutor({ d1: undefined as never }),
        /d1 binding is required/,
    );
});

test("D1 read executor without plans returns plan-missing diagnostic", async () => {
    const executor = createD1ReadOperationExecutor(fakeD1);
    const request = buildOperationRequest(
        { operationId: "user.findMany", contractFingerprint: "fp" },
        declaredVosOperation("User.collect()"),
    );

    const read = await executor.execute(request);
    assert.equal(read.ok, false);
    if (!read.ok) {
        assert.equal(read.diagnostics[0]?.code, "IRIS-D1-PLAN-MISSING");
    }
});

test("D1 read executor runs build-time plan and maps wire columns", async () => {
    const executor = createD1ReadOperationExecutor(fakeD1, plans, wireNames);
    const read = await executor.execute(
        buildOperationRequest(
            { operationId: "User.findMany", contractFingerprint: "fp" },
            declaredVosOperation("User.collect()"),
        ),
    );
    assert.equal(read.ok, true);
    if (read.ok) {
        assert.equal(read.value[0]?.userId, "u1");
        assert.equal(read.value[0]?.userName, "Ada");
        assert.equal(read.value[0]?.active, 1);
    }
    assert.equal(fakeD1.lastSql, plans["User.findMany"].sql);
});

test("D1 read executor rejects mutation operations", async () => {
    const executor = createD1ReadOperationExecutor(fakeD1, plans, wireNames);
    const write = await executor.executeUnit(
        buildOperationRequest(
            { operationId: "User.create", contractFingerprint: "fp" },
            declaredVosOperation("User.insert({})"),
        ),
    );
    assert.equal(write.ok, false);
    if (!write.ok) {
        assert.equal(write.diagnostics[0]?.code, "IRIS-D1-WRITE-NOT-WIRED");
    }
});

test("createIrisOperationExecutor applies contract fingerprint guard", async () => {
    const executor = await createIrisOperationExecutor({
        d1: fakeD1,
        plans,
        wireNamesByEntity: wireNames,
        contractFingerprint: "expected-fp",
    });
    const mismatch = await executor.execute(
        buildOperationRequest(
            { operationId: "User.findMany", contractFingerprint: "wrong-fp" },
            declaredVosOperation("User.collect()"),
        ),
    );
    assert.equal(mismatch.ok, false);
    if (!mismatch.ok) {
        assert.equal(mismatch.diagnostics[0]?.code, "IRIS-CONTRACT-MISMATCH");
    }
});
