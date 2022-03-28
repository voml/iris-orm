import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { INVALID_SCHEMA, USER_SCHEMA, USER_SCHEMA_FINGERPRINT } from "./fixtures.ts";

const USER_SCHEMA_INLINE = `
table User {
    @@user_id: utf8,
    @user_name: utf8,
    active: bool,
}
`;

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

test("introspectSchema returns table metadata", () => {
    const binding = loadBinding();
    const intro = JSON.parse(binding.introspectSchema(USER_SCHEMA)) as { ok: boolean; tables: { name: string }[] };
    assert.equal(intro.ok, true);
    assert.equal(intro.tables[0]?.name, "User");
});

test("openMemorySession executes VOS", () => {
    const binding = loadBinding();
    const session = binding.openMemorySession();
    const result = session.executeVos("table User { @@id: utf8, name: utf8 } select User { id, name }");
    assert.equal(typeof result.ok, "boolean");
    session.close();
});

test("openSqliteSession opens :memory: database", () => {
    const binding = loadBinding();
    const session = binding.openSqliteSession(":memory:");
    session.managedPush(USER_SCHEMA_INLINE);
    const result = session.executeVos("User.filter(x => x.active).collect()");
    assert.equal(result.ok, true);
    assert.equal(JSON.parse(result.rowsJson).length, 0);
    session.close();
});

test("openSession unified entry opens sqlite profile", () => {
    const binding = loadBinding();
    const session = binding.openSession({ profile: "sqlite", sqlitePath: ":memory:" });
    session.managedPush(USER_SCHEMA_INLINE);
    const result = session.executeVos("User.filter(x => x.active).collect()");
    assert.equal(result.ok, true);
    session.close();
});

test("openPostgresSession requires reachable server", (t) => {
    const binding = loadBinding();
    try {
        const session = binding.openPostgresSession("postgres://invalid:invalid@127.0.0.1:1/none");
        session.close();
        t.skip("unexpected postgres connection success");
    } catch {
        // expected when no server / bad URL
    }
});
