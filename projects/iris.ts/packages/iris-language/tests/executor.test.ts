import assert from "node:assert/strict";
import { test } from "node:test";

import { isWasmCoreMissing, srcImport } from "./fixtures.ts";

test("createBrowserIrisDbBinding forwards parameters to the Rust binding", async (t) => {
    const language = await import(srcImport("src/index.ts"));
    language.resetInitStateForTests();

    try {
        await language.initIris();
    } catch (error) {
        if (isWasmCoreMissing(error)) {
            t.skip("browser semantic core not built");
            return;
        }
        throw error;
    }

    let binding;
    try {
        binding = await language.createIrisDbBinding();
    } catch (error) {
        if (isWasmCoreMissing(error)) {
            t.skip("browser semantic core not built");
            return;
        }
        throw error;
    }

    // Missing parameter bindings are rejected by the Rust binder before execution.
    await assert.rejects(
        () => binding.query("User.filter(x => x.active == $active).collect()", {}),
        /unbound/i,
    );

    // Bound parameters reach Rust, which reports the empty reference store.
    await assert.rejects(
        () => binding.query("User.filter(x => x.active == $active).collect()", { active: true }),
        /unknown table/i,
    );

    await binding.close();
});

test("createIrisDbBinding matches the node binding surface", async (t) => {
    const language = await import(srcImport("src/index.ts"));
    language.resetInitStateForTests();

    try {
        await language.initIris();
    } catch (error) {
        if (isWasmCoreMissing(error)) {
            t.skip("browser semantic core not built");
            return;
        }
        throw error;
    }

    let binding;
    try {
        binding = await language.createIrisDbBinding();
    } catch (error) {
        if (isWasmCoreMissing(error)) {
            t.skip("browser semantic core not built");
            return;
        }
        throw error;
    }

    assert.equal(typeof binding.query, "function");
    assert.equal(typeof binding.execute, "function");
    assert.equal(typeof binding.close, "function");
    await binding.close();
});

test("createIrisOperationExecutor matches the binding surface", async (t) => {
    const language = await import(srcImport("src/index.ts"));
    language.resetInitStateForTests();

    try {
        await language.initIris();
    } catch (error) {
        if (isWasmCoreMissing(error)) {
            t.skip("browser semantic core not built");
            return;
        }
        throw error;
    }

    const executor = await language.createIrisOperationExecutor();
    assert.equal(typeof executor.execute, "function");
    assert.equal(typeof executor.executeUnit, "function");
    await executor.close();
});
