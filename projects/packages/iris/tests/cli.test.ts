import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const pkgRoot = fileURLToPath(new URL("..", import.meta.url));
const cli = join(pkgRoot, "bin/iris.ts");
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

test("iris doctor hides platform package names", () => {
    const doctor = runCli(["doctor"]);
    assert.equal(doctor.status, 0);
    assert.match(doctor.stdout, /@yydb\/iris\/node/);
    assert.match(doctor.stdout, /Semantic cores/);
    assert.doesNotMatch(doctor.stdout, /@yydb\/iris-win32-x64/);
    assert.doesNotMatch(doctor.stdout, /@yydb\/iris-linux-x64/);
    assert.doesNotMatch(doctor.stdout, /@yydb\/iris-unknown-wasm32/);
});

test("iris generate defaults to typescript target", () => {
    const help = runCli(["generate", "--help"]);
    assert.equal(help.status, 0);
    assert.match(help.stdout, /typescript/);
});

test("iris push command is registered", () => {
    const help = runCli(["--help"]);
    assert.equal(help.status, 0);
    assert.match(help.stdout, /push/);
});
