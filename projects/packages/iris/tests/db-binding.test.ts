import assert from "node:assert/strict";
import { test } from "node:test";

import type { MemorySessionBinding } from "../src/bindings.ts";
import { createIrisDbBindingFromSession } from "../src/runtime/db-binding.ts";

function stubSession(calls: { query: string[]; execute: string[] }): MemorySessionBinding {
    return {
        query: (source) => {
            calls.query.push(source);
            return { ok: true, rowsJson: '[{"id":1}]' };
        },
        execute: (source) => {
            calls.execute.push(source);
            return { ok: true, rowsJson: "[]" };
        },
        executeVos: (source) => {
            calls.query.push(`legacy:${source}`);
            return { ok: true, rowsJson: "[]" };
        },
        close: () => {},
    };
}

test("createIrisDbBindingFromSession routes query and execute separately", async () => {
    const calls = { query: [] as string[], execute: [] as string[] };
    const binding = createIrisDbBindingFromSession(stubSession(calls));

    const rows = await binding.query("User.collect()");
    assert.deepEqual(rows, [{ id: 1 }]);
    assert.deepEqual(calls.query, ["User.collect()"]);

    await binding.execute("User.collect()");
    assert.deepEqual(calls.execute, ["User.collect()"]);

    await binding.close();
});

test("createIrisDbBindingFromSession falls back to executeVos when query is absent", async () => {
    const calls: string[] = [];
    const binding = createIrisDbBindingFromSession({
        executeVos: (source) => {
            calls.push(source);
            return { ok: true, rowsJson: "[]" };
        },
        close: () => {},
    });

    await binding.query("Post.collect()");
    assert.deepEqual(calls, ["Post.collect()"]);
});
