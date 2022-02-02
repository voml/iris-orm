import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { INVALID_SCHEMA, USER_SCHEMA } from "./fixtures.ts";
import { ensureNativeOverride } from "./helpers.ts";

const pkgRoot = fileURLToPath(new URL("..", import.meta.url));
const cli = join(pkgRoot, "src/node/cli.ts");
const nodeBin = process.execPath;

function runCli(args: string[]) {
    return spawnSync(nodeBin, ["--experimental-strip-types", cli, ...args], {
        encoding: "utf8",
    });
}

test("iris --help", () => {
    const help = runCli(["--help"]);
    assert.equal(help.status, 0);
    assert.match(help.stdout, /iris/);
});

test("iris doctor reports node facade", () => {
    ensureNativeOverride();
    const doctor = runCli(["doctor"]);
    assert.equal(doctor.status, 0);
    assert.match(doctor.stdout, /@yydb\/iris\/node/);
});

test("iris check validates schema via Rust N-API", (t) => {
    ensureNativeOverride();
    const probe = runCli(["doctor"]);
    if (probe.stdout.includes("not loaded")) {
        t.skip("N-API platform package not available");
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
    ensureNativeOverride();
    const probe = runCli(["doctor"]);
    if (probe.stdout.includes("not loaded")) {
        t.skip("N-API platform package not available");
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
