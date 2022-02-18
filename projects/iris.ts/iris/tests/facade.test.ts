import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

import { INVALID_SCHEMA, USER_SCHEMA, USER_SCHEMA_FINGERPRINT } from "./fixtures.ts";
import { srcImport } from "./helpers.ts";

function ensureNativeOverride(): void {
    if (process.env.NAPI_RS_NATIVE_LIBRARY_PATH) {
        return;
    }
    const resolveScript = fileURLToPath(new URL("../../iris-napi/scripts/resolve-platform-dir.mjs", import.meta.url));
    const artifactOut = spawnSync(process.execPath, [resolveScript, "--artifact"], {
        encoding: "utf8",
    });
    if (artifactOut.status === 0 && artifactOut.stdout.trim()) {
        process.env.NAPI_RS_NATIVE_LIBRARY_PATH = artifactOut.stdout.trim();
    }
}

test("createIris throws wasm-not-initialized before initIris", async () => {
    const browser = await import(srcImport("src/browser/index.ts"));
    const wasm = await import(srcImport("src/browser/wasm.ts"));
    wasm.resetInitStateForTests();

    await assert.rejects(
        () => browser.createIris(),
        (error: unknown) => error instanceof Error && "code" in error && error.code === "wasm-not-initialized",
    );
});

test("browser createIris exposes symmetric runtime methods", async (t) => {
    const browser = await import(srcImport("src/browser/index.ts"));
    const wasm = await import(srcImport("src/browser/wasm.ts"));
    wasm.resetInitStateForTests();

    try {
        await wasm.initIris();
    } catch (error) {
        if (error instanceof Error && "code" in error && error.code === "wasm-package-missing") {
            t.skip("browser semantic core not built");
            return;
        }
        throw error;
    }

    const runtime = await browser.createIris();
    assert.equal(runtime.host, "web");
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

test("node createIris exposes symmetric runtime methods", async (t) => {
    ensureNativeOverride();
    const node = await import(srcImport("src/node/index.ts"));

    let runtime;
    try {
        runtime = await node.createIris();
    } catch (error) {
        if (error instanceof Error && "code" in error && error.code === "native-package-missing") {
            t.skip("Node semantic core not installed");
            return;
        }
        throw error;
    }

    assert.equal(runtime.host, "node");
    assert.ok(runtime.version());

    const ok = runtime.checkSource(USER_SCHEMA);
    assert.equal(ok.ok, true);
    assert.equal(ok.schemaFingerprint, USER_SCHEMA_FINGERPRINT);

    const intro = runtime.introspectSchema(USER_SCHEMA);
    assert.equal(intro.ok, true);
    assert.equal(intro.tables[0]?.name, "User");

    const session = runtime.openSession();
    session.close();
});

test("node createIris opens sqlite session via sqlitePath", async (t) => {
    ensureNativeOverride();
    const node = await import(srcImport("src/node/index.ts"));

    let runtime;
    try {
        runtime = await node.createIris();
    } catch (error) {
        if (error instanceof Error && "code" in error && error.code === "native-package-missing") {
            t.skip("Node semantic core not installed");
            return;
        }
        throw error;
    }

    const session = runtime.openSession({ sqlitePath: ":memory:" });
    const result = session.execute("User.filter(x => x.active).collect()");
    assert.equal(typeof result.ok, "boolean");
    session.close();
});

test("@yydb/iris/types does not export a TS checkSource implementation", async () => {
    const types = await import(srcImport("src/types/index.ts"));
    assert.equal("checkSource" in types, false);
});

test("node createIris opens sqlite session via openSession", async (t) => {
    ensureNativeOverride();
    const node = await import(srcImport("src/node/index.ts"));

    let runtime;
    try {
        runtime = await node.createIris();
    } catch (error) {
        if (error instanceof Error && "code" in error && error.code === "native-package-missing") {
            t.skip("Node semantic core not installed");
            return;
        }
        throw error;
    }

    const session = runtime.openSession({ profile: "sqlite", sqlitePath: ":memory:" });
    assert.equal(typeof session.execute, "function");
    session.close();
});

test("openDatasourceSession opens foreign adapter session", async (t) => {
    ensureNativeOverride();
    const node = await import(srcImport("src/node/index.ts"));

    try {
        const session = await node.openDatasourceSession({ profile: "memory" });
        session.close();
    } catch (error) {
        if (error instanceof Error && "code" in error && error.code === "native-package-missing") {
            t.skip("Node semantic core not installed");
            return;
        }
        throw error;
    }
});

test("@yydb/iris/node exposes host facade only", async () => {
    const node = await import(srcImport("src/node/index.ts"));
    assert.equal("createIris" in node, true);
    assert.equal("createIrisDbBinding" in node, true);
    assert.equal("createIrisExecutor" in node, true);
    assert.equal("checkSchemaFile" in node, true);
    assert.equal("printDoctorReport" in node, true);
    assert.equal("openDatasourceSession" in node, true);
    assert.equal("loadSemanticCore" in node, false);
    assert.equal("loadNativeBinding" in node, false);
    assert.equal("resolvePlatformPackageName" in node, false);
});
