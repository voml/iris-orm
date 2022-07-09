import assert from "node:assert/strict";
import { test } from "node:test";

import {
    applyD1ReturningProjection,
    bindD1Parameters,
    planLookupParamKeys,
    resolveD1Plan,
} from "../src/cloudflare/d1-plan.ts";

const plans = {
    "User.findMany": { sql: "SELECT user_id FROM User", mode: "read" as const },
    "User.findMany@take": {
        sql: "SELECT user_id FROM User LIMIT ?",
        mode: "read" as const,
        paramOrder: ["take"],
    },
    "User.findMany@p_active,take": {
        sql: "SELECT user_id FROM User WHERE active = ? LIMIT ?",
        mode: "read" as const,
        paramOrder: ["p_active", "take"],
    },
    "User.findUnique@p_user_id": {
        sql: "SELECT user_id FROM User WHERE user_id = ? LIMIT 1",
        mode: "read" as const,
        paramOrder: ["p_user_id"],
    },
    "User.create@data_user_id": {
        sql: "INSERT INTO User (user_id) VALUES (?) RETURNING user_id",
        mode: "write-returning" as const,
        paramOrder: ["data_user_id"],
    },
    "User.create@data_active,data_user_id,data_user_name": {
        sql: "INSERT INTO User (user_id, user_name, active) VALUES (?, ?, ?) RETURNING user_id, user_name, active",
        mode: "write-returning" as const,
        paramOrder: ["data_user_id", "data_user_name", "data_active"],
    },
};

test("planLookupParamKeys excludes select_cols metadata", () => {
    assert.deepEqual(
        planLookupParamKeys({ data_user_id: "u1", select_cols: "user_id,user_name" }),
        ["data_user_id"],
    );
});

test("applyD1ReturningProjection rewrites RETURNING columns", () => {
    const sql = "INSERT INTO User (user_id, user_name) VALUES (?, ?) RETURNING user_id, user_name, active";
    assert.equal(
        applyD1ReturningProjection(sql, "user_id,user_name"),
        "INSERT INTO User (user_id, user_name) VALUES (?, ?) RETURNING user_id,user_name",
    );
});

test("resolveD1Plan ignores select_cols when matching variants", () => {
    const plan = resolveD1Plan(plans, "User.create", {
        data_user_id: "u1",
        data_user_name: "Ada",
        data_active: true,
        select_cols: "user_id,user_name",
    });
    assert.equal(plan?.sql, plans["User.create@data_active,data_user_id,data_user_name"].sql);
});

test("resolveD1Plan matches sorted multi-parameter variant keys", () => {
    const plan = resolveD1Plan(plans, "User.findMany", { take: 5, p_active: true });
    assert.equal(plan?.sql, plans["User.findMany@p_active,take"].sql);
    assert.deepEqual(bindD1Parameters(plan!, { take: 5, p_active: true }), [1, 5]);
});

test("resolveD1Plan falls back to base plan without parameters", () => {
    assert.equal(resolveD1Plan(plans, "User.findMany")?.sql, plans["User.findMany"].sql);
});

test("resolveD1Plan resolves findUnique primary-key variants", () => {
    const plan = resolveD1Plan(plans, "User.findUnique", { p_user_id: "u1" });
    assert.equal(plan?.sql, plans["User.findUnique@p_user_id"].sql);
    assert.deepEqual(bindD1Parameters(plan!, { p_user_id: "u1" }), ["u1"]);
});

test("resolveD1Plan resolves partial create subsets", () => {
    const plan = resolveD1Plan(plans, "User.create", { data_user_id: "u1" });
    assert.equal(plan?.mode, "write-returning");
    assert.match(plan?.sql ?? "", /INSERT INTO User \(user_id\)/);
    assert.deepEqual(bindD1Parameters(plan!, { data_user_id: "u1" }), ["u1"]);
});

test("resolveD1Plan resolves update patch and where variants", () => {
    const updatePlans = {
        "User.update@p_user_id,patch_user_name": {
            sql: "UPDATE User SET user_name = ? WHERE user_id = ? RETURNING user_id",
            mode: "write-returning" as const,
            paramOrder: ["patch_user_name", "p_user_id"],
        },
    };
    const plan = resolveD1Plan(updatePlans, "User.update", {
        patch_user_name: "Carol",
        p_user_id: "u1",
    });
    assert.equal(plan?.mode, "write-returning");
    assert.deepEqual(bindD1Parameters(plan!, { patch_user_name: "Carol", p_user_id: "u1" }), ["Carol", "u1"]);
});

test("resolveD1Plan resolves delete primary-key variants", () => {
    const deletePlans = {
        "User.delete@p_user_id": {
            sql: "DELETE FROM User WHERE user_id = ?",
            mode: "write" as const,
            paramOrder: ["p_user_id"],
        },
    };
    const plan = resolveD1Plan(deletePlans, "User.delete", { p_user_id: "u1" });
    assert.equal(plan?.mode, "write");
    assert.deepEqual(bindD1Parameters(plan!, { p_user_id: "u1" }), ["u1"]);
});

test("resolveD1Plan resolves create data parameter variants", () => {
    const plan = resolveD1Plan(plans, "User.create", {
        data_user_name: "Ada",
        data_user_id: "u1",
        data_active: true,
    });
    assert.equal(plan?.mode, "write-returning");
    assert.match(plan?.sql ?? "", /INSERT INTO User/);
    assert.deepEqual(bindD1Parameters(plan!, {
        data_user_id: "u1",
        data_user_name: "Ada",
        data_active: true,
    }), ["u1", "Ada", 1]);
});
