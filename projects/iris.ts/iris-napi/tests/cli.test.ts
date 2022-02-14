import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { tmpdir } from "node:os";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { INVALID_SCHEMA, USER_SCHEMA } from "./fixtures.ts";

const require = createRequire(import.meta.url);
const pkgRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const irisCli = join(pkgRoot, "../iris/bin/iris.ts");
const nodeBin = process.execPath;

function ensureNativeOverride(): void {
    if (process.env.NAPI_RS_NATIVE_LIBRARY_PATH) {
        return;
    }
    const artifactOut = spawnSync(process.execPath, [join(pkgRoot, "scripts", "resolve-platform-dir.mjs"), "--artifact"], {
        encoding: "utf8",
    });
    if (artifactOut.status === 0 && artifactOut.stdout.trim()) {
        process.env.NAPI_RS_NATIVE_LIBRARY_PATH = artifactOut.stdout.trim();
    }
}

function runCli(args: string[]) {
    ensureNativeOverride();
    return spawnSync(nodeBin, ["--experimental-strip-types", irisCli, ...args], {
        encoding: "utf8",
        env: process.env,
    });
}

test("iris check validates schema via Rust N-API", (t) => {
    try {
        require(process.env.NAPI_RS_NATIVE_LIBRARY_PATH ?? "");
    } catch {
        t.skip("N-API artifact not built");
        return;
    }

    const tmp = mkdtempSync(join(tmpdir(), "iris-cli-"));
    const schema = join(tmp, "user.iris");
    writeFileSync(schema, USER_SCHEMA);
    const check = runCli(["check", schema]);
    rmSync(tmp, { recursive: true, force: true });

    assert.equal(check.status, 0);
    assert.match(check.stdout, /iris check: ok/);
    assert.doesNotMatch(check.stdout, /vos-parser/);
});

test("iris check rejects invalid schema", (t) => {
    try {
        require(process.env.NAPI_RS_NATIVE_LIBRARY_PATH ?? "");
    } catch {
        t.skip("N-API artifact not built");
        return;
    }

    const tmp = mkdtempSync(join(tmpdir(), "iris-cli-"));
    const schema = join(tmp, "bad.iris");
    writeFileSync(schema, INVALID_SCHEMA);
    const check = runCli(["check", schema]);
    rmSync(tmp, { recursive: true, force: true });

    assert.equal(check.status, 1);
    assert.match(check.stderr + check.stdout, /error:/);
});
