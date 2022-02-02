import assert from "node:assert/strict";
import { test } from "node:test";

import { INVALID_SCHEMA, USER_SCHEMA, USER_SCHEMA_FINGERPRINT } from "./fixtures.ts";
import { ensureNativeOverride, loadWasmBinding, srcImport, summarizeCheck, wasmArtifactBuilt } from "./helpers.ts";

test("wasm binding validates USER_SCHEMA fixture", async (t) => {
    if (!wasmArtifactBuilt()) {
        t.skip("run pnpm run wasm:build first");
        return;
    }

    const wasm = await loadWasmBinding();
    const version = wasm.irisVersion();
    assert.ok(version && version !== "0.0.0");

    const result = wasm.checkSource(USER_SCHEMA);
    assert.equal(result.ok, true);
    assert.equal(result.tableCount, 1);
    assert.equal(result.schemaFingerprint, USER_SCHEMA_FINGERPRINT);
    assert.equal(result.generatorVersion, version);

    const bad = wasm.checkSource(INVALID_SCHEMA);
    assert.equal(bad.ok, false);
});

test("loadNativeBinding loads workspace N-API artifact", async (t) => {
    ensureNativeOverride();
    const native = await import(srcImport("src/node/native.ts"));
    native.resetNativeBindingCacheForTests();

    let binding;
    try {
        binding = await native.loadNativeBinding();
    } catch (error) {
        if (error instanceof Error && "code" in error && error.code === "native-package-missing") {
            t.skip("optional @yydb/iris-win32-x64 not installed");
            return;
        }
        throw error;
    }

    const version = binding.module.irisVersion();
    assert.ok(version && version !== "0.0.0");
    assert.deepEqual(summarizeCheck(binding.module.checkSource(USER_SCHEMA)), {
        ok: true,
        tableCount: 1,
        schemaFingerprint: USER_SCHEMA_FINGERPRINT,
        generatorVersion: version,
        error: null,
    });
});

test("N-API and WASM agree on version and fingerprint", async (t) => {
    if (!wasmArtifactBuilt()) {
        t.skip("run pnpm run wasm:build first");
        return;
    }

    ensureNativeOverride();
    const wasm = await loadWasmBinding();
    const wasmCheck = summarizeCheck(wasm.checkSource(USER_SCHEMA));

    const native = await import(srcImport("src/node/native.ts"));
    native.resetNativeBindingCacheForTests();

    let binding;
    try {
        binding = await native.loadNativeBinding();
    } catch (error) {
        if (error instanceof Error && "code" in error && error.code === "native-package-missing") {
            t.skip("optional @yydb/iris-win32-x64 not installed");
            return;
        }
        throw error;
    }

    const napiCheck = summarizeCheck(binding.module.checkSource(USER_SCHEMA));
    assert.equal(binding.module.irisVersion(), wasm.irisVersion());
    assert.equal(napiCheck.schemaFingerprint, wasmCheck.schemaFingerprint);
    assert.equal(napiCheck.tableCount, wasmCheck.tableCount);
});

test("initIris loads WASM through browser facade", async (t) => {
    if (!wasmArtifactBuilt()) {
        t.skip("run pnpm run wasm:build first");
        return;
    }

    const browser = await import(srcImport("src/browser/wasm.ts"));
    browser.resetWasmStateForTests();
    await browser.initIris();

    const binding = browser.getWasmBindingForTests();
    assert.ok(binding);
    const version = binding.irisVersion();
    const result = summarizeCheck(binding.checkSource(USER_SCHEMA));
    assert.equal(result.ok, true);
    assert.equal(result.schemaFingerprint, USER_SCHEMA_FINGERPRINT);
    assert.equal(result.generatorVersion, version);
});

test("createIris stays gated after WASM init", async (t) => {
    if (!wasmArtifactBuilt()) {
        t.skip("run pnpm run wasm:build first");
        return;
    }

    const browser = await import(srcImport("src/browser/index.ts"));
    const wasm = await import(srcImport("src/browser/wasm.ts"));
    wasm.resetWasmStateForTests();
    await wasm.initIris();

    await assert.rejects(
        () => browser.createIris(),
        (error: unknown) => error instanceof Error && "code" in error && error.code === "wasm-not-implemented",
    );
});

test("@yydb/iris/types does not export a TS checkSource implementation", async () => {
    const types = await import(srcImport("src/types/index.ts"));
    assert.equal("checkSource" in types, false);
});

test("native binding rejects invalid schema", async (t) => {
    ensureNativeOverride();
    const native = await import(srcImport("src/node/native.ts"));
    native.resetNativeBindingCacheForTests();

    let binding;
    try {
        binding = await native.loadNativeBinding();
    } catch (error) {
        if (error instanceof Error && "code" in error && error.code === "native-package-missing") {
            t.skip("optional @yydb/iris-win32-x64 not installed");
            return;
        }
        throw error;
    }

    const bad = binding.module.checkSource(INVALID_SCHEMA);
    assert.equal(bad.ok, false);
    assert.equal(bad.tableCount, 0);
});
