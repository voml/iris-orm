import assert from "node:assert/strict";
import { test } from "node:test";

import { bindD1Parameters, resolveD1Plan } from "../src/cloudflare/d1-plan.ts";

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
};

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
