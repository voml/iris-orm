import assert from "node:assert/strict";
import { test } from "node:test";

import { ensureNativeOverride, srcImport, wasmPlatformPackageInstalled } from "./helpers.ts";

test("loadNativeBinding loads optional platform package", async (t) => {
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

    assert.ok(binding.packageName);
    assert.ok(binding.module.irisVersion());
});

test("initIris delegates to optional WASM platform package", async (t) => {
    if (!wasmPlatformPackageInstalled()) {
        t.skip("run pnpm run wasm:build first");
        return;
    }

    const wasm = await import(srcImport("src/browser/wasm.ts"));
    wasm.resetWasmStateForTests();
    await wasm.initIris();
    assert.ok(wasm.getWasmBindingForTests());
});

test("createIris throws wasm-not-initialized before initIris", async () => {
    const browser = await import(srcImport("src/browser/index.ts"));
    const wasm = await import(srcImport("src/browser/wasm.ts"));
    wasm.resetWasmStateForTests();

    await assert.rejects(
        () => browser.createIris(),
        (error: unknown) =>
            error instanceof Error && "code" in error && error.code === "wasm-not-initialized",
    );
});

test("createIris stays gated after initIris", async (t) => {
    if (!wasmPlatformPackageInstalled()) {
        t.skip("run pnpm run wasm:build first");
        return;
    }

    const browser = await import(srcImport("src/browser/index.ts"));
    const wasm = await import(srcImport("src/browser/wasm.ts"));
    wasm.resetWasmStateForTests();
    await wasm.initIris();

    await assert.rejects(
        () => browser.createIris(),
        (error: unknown) =>
            error instanceof Error && "code" in error && error.code === "wasm-not-implemented",
    );
});

test("@yydb/iris/types does not export a TS checkSource implementation", async () => {
    const types = await import(srcImport("src/types/index.ts"));
    assert.equal("checkSource" in types, false);
});
