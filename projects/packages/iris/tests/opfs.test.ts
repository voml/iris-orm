import assert from "node:assert/strict";
import { test } from "node:test";

import {
    createIrisOperationExecutor,
    createSqlitePlanOperationExecutor,
    openOpfsStore,
} from "../src/opfs/index.ts";
import { IrisFacadeError } from "../src/types/errors.ts";
import { buildOperationRequest, declaredVosOperation } from "../src/runtime/build-operation-request.ts";
import { negotiateCapabilities } from "../src/runtime/negotiate-capabilities.ts";
import { resolveBindingProfile } from "../src/runtime/open-binding-session.ts";
import { resolveStorageProfile } from "../src/types/profile.ts";

const fakeSqlite = {
    lastSql: "",
    lastBind: [] as unknown[],
    prepare(query: string) {
        this.lastSql = query;
        return {
            bind(...values: unknown[]) {
                fakeSqlite.lastBind = values;
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
};

test("resolveBindingProfile infers opfs from opfsPath", () => {
    assert.equal(resolveBindingProfile({ opfsPath: "app" }), "opfs");
    assert.equal(resolveStorageProfile("opfs"), "opfs");
});

test("negotiateCapabilities maps browser opfs profile to local-durable storage", () => {
    const caps = negotiateCapabilities({ host: "browser", profile: "opfs", bindingReady: true });
    assert.equal(caps.profile, "opfs");
    assert.equal(caps.durability, "local-durable");
    assert.equal(caps.storage.opfs, true);
});

test("createSqlitePlanOperationExecutor runs read and write plans", async () => {
    const executor = createSqlitePlanOperationExecutor(fakeSqlite, plans, wireNames);
    const read = await executor.execute(
        buildOperationRequest(
            { operationId: "User.findMany", contractFingerprint: "fp" },
            declaredVosOperation("User.collect()"),
        ),
    );
    assert.equal(read.ok, true);
    if (read.ok) {
        assert.deepEqual(read.value, [{ userId: "u1", userName: "Ada", active: 1 }]);
    }

    const create = await executor.execute(
        buildOperationRequest(
            { operationId: "User.create", contractFingerprint: "fp" },
            declaredVosOperation(
                "User::insert({ user_id: $data_user_id, user_name: $data_user_name, active: $data_active })",
            ),
            {
                data_user_id: "u2",
                data_user_name: "Grace",
                data_active: true,
            },
        ),
    );
    assert.equal(create.ok, true);
    assert.match(fakeSqlite.lastSql, /INSERT INTO User/);

    const del = await executor.executeUnit(
        buildOperationRequest(
            { operationId: "User.delete", contractFingerprint: "fp" },
            declaredVosOperation("User.filter(x => x.user_id == $p_user_id).delete()"),
            { p_user_id: "u1" },
        ),
    );
    assert.equal(del.ok, true);
    assert.match(fakeSqlite.lastSql, /DELETE FROM User/);
});

test("createIrisOperationExecutor requires sqlite handle", async () => {
    await assert.rejects(
        () => createIrisOperationExecutor({ profile: "opfs", sqlite: undefined as never }),
        /sqlite handle is required/,
    );
});

test("openOpfsStore rejects hosts without OPFS", async () => {
    await assert.rejects(
        () => openOpfsStore({ name: "app" }),
        (error: unknown) => error instanceof IrisFacadeError && error.code === "opfs-unavailable",
    );
});
