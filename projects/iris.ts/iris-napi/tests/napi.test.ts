import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { INVALID_SCHEMA, USER_SCHEMA, USER_SCHEMA_FINGERPRINT } from "./fixtures.ts";

const require = createRequire(import.meta.url);
const pkgRoot = join(dirname(fileURLToPath(import.meta.url)), "..");

function loadBinding() {
    const artifactOut = spawnSync(process.execPath, [join(pkgRoot, "scripts", "resolve-platform-dir.mjs"), "--artifact"], {
        encoding: "utf8",
    });
    if (artifactOut.status !== 0) {
        throw new Error(artifactOut.stderr || artifactOut.stdout);
    }
    const bindingPath = process.env.NAPI_RS_NATIVE_LIBRARY_PATH ?? artifactOut.stdout.trim();
    return require(bindingPath);
}

test("irisVersion returns workspace semver", () => {
    const binding = loadBinding();
    const version = binding.irisVersion();
    assert.ok(version && version !== "0.0.0");
});

test("checkSource validates USER_SCHEMA fixture", () => {
    const binding = loadBinding();
    const result = binding.checkSource(USER_SCHEMA);
    assert.equal(result.ok, true);
    assert.equal(result.tableCount, 1);
    assert.equal(result.schemaFingerprint, USER_SCHEMA_FINGERPRINT);
    assert.equal(result.generatorVersion, binding.irisVersion());
});

test("checkSource rejects invalid schema", () => {
    const binding = loadBinding();
    const bad = binding.checkSource(INVALID_SCHEMA);
    assert.equal(bad.ok, false);
});
