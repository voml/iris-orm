import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const pkgRoot = fileURLToPath(new URL("..", import.meta.url));
const cli = join(pkgRoot, "bin/iris.mjs");
const nodeBin = process.execPath;

function runCli(args: string[], cwd?: string) {
    return spawnSync(nodeBin, [cli, ...args], {
        cwd,
        encoding: "utf8",
    });
}

async function writeThrowingConfig(dir: string): Promise<string> {
    const marker = join(dir, "config-evaluated.txt");
    const configPath = join(dir, "iris.config.mts");
    await writeFile(
        configPath,
        [
            `import { writeFileSync } from "node:fs";`,
            `writeFileSync(${JSON.stringify(marker)}, "evaluated");`,
            `throw new Error("config boom");`,
        ].join("\n"),
        "utf8",
    );
    return marker;
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
    assert.match(doctor.stdout, /Semantic core/);
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

test("help, version, and invalid argv do not evaluate project config", async () => {
    const dir = await mkdtemp(join(tmpdir(), "iris-cli-no-config-"));
    const marker = await writeThrowingConfig(dir);

    const help = runCli(["--help"], dir);
    assert.equal(help.status, 0);

    const version = runCli(["--version"], dir);
    assert.equal(version.status, 0);

    const invalid = runCli(["no-such-command"], dir);
    assert.equal(invalid.status, 2);

    assert.equal(existsSync(marker), false);
});

test("invalid option exits with code 2", () => {
    const result = runCli(["check", "--no-such-option"]);
    assert.equal(result.status, 2);
});

test("config execution failure exits 1 with one structured diagnostic", async () => {
    const dir = await mkdtemp(join(tmpdir(), "iris-cli-bad-config-"));
    const marker = await writeThrowingConfig(dir);

    const result = runCli(["check", "--config", dir]);
    assert.equal(result.status, 1);
    assert.equal(existsSync(marker), true);
    assert.match(result.stderr, /config-load-failed/);
    assert.match(result.stderr, /iris\.config\.mts/);
    assert.match(result.stderr, /config boom/);
    assert.equal(result.stderr.match(/config-load-failed/g)?.length, 1);
});
