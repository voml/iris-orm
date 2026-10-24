import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";

import { USER_SCHEMA, USER_SCHEMA_FINGERPRINT, isWasmCoreMissing, srcImport } from "./fixtures.ts";

test("createIris throws wasm-not-initialized before initIris", async () => {
    const language = await import(srcImport("src/index.ts"));
    language.resetInitStateForTests();

    await assert.rejects(
        () => language.createIris(),
        (error: unknown) => error instanceof Error && "code" in error && error.code === "wasm-not-initialized",
    );
});

test("browser createIris exposes symmetric runtime methods", async (t) => {
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

    const runtime = await language.createIris();
    assert.equal(runtime.host, "browser");
    assert.ok(runtime.version());

    const ok = runtime.checkSource(USER_SCHEMA);
    assert.equal(ok.ok, true);
    assert.equal(ok.schemaFingerprint, USER_SCHEMA_FINGERPRINT);

    const intro = runtime.introspectSchema(USER_SCHEMA);
    assert.equal(intro.ok, true);
    assert.equal(intro.schemaFingerprint, USER_SCHEMA_FINGERPRINT);
    assert.equal(intro.tables.length, 1);

    const session = runtime.openSession();
    session.close();
});

test("loadIrisWeb caches the active semantic core", async (t) => {
    const language = await import(srcImport("src/index.ts"));
    language.resetInitStateForTests();

    let first;
    try {
        first = await language.loadIrisWeb();
    } catch (error) {
        if (isWasmCoreMissing(error)) {
            t.skip("browser semantic core not built");
            return;
        }
        throw error;
    }

    assert.equal(await language.loadIrisWeb(), first);
    assert.equal(language.getWasmSemanticCore(), first);
    assert.equal(first.irisVersion(), language.irisVersion());
});

test("language entry re-exports platform WASM capabilities", async () => {
    const language = await import(srcImport("src/index.ts"));
    for (const name of [
        "initWasm",
        "irisVersion",
        "checkSource",
        "introspectSchema",
        "executeVosMemory",
        "openMemorySession",
        "loadIrisWasm",
        "loadIrisWeb",
        "initIris",
        "getWasmSemanticCore",
        "isBrowserSemanticCoreInstalled",
        "createIris",
        "createIrisDbBinding",
        "createIrisOperationExecutor",
        "createBrowserIrisDbBinding",
        "openLocalStore",
    ]) {
        assert.equal(name in language, true, `missing export ${name}`);
    }
});

test("isBrowserSemanticCoreInstalled resolves the workspace platform package", async () => {
    const language = await import(srcImport("src/index.ts"));
    assert.equal(language.isBrowserSemanticCoreInstalled(), true);
});

test("src/wasm.ts stays browser-safe (no node:module import)", async () => {
    const source = await readFile(new URL("../src/wasm.ts", import.meta.url), "utf8");
    assert.doesNotMatch(source, /from\s+["']node:module["']/);
    assert.doesNotMatch(source, /require\(\s*["']node:module["']\s*\)/);
});
