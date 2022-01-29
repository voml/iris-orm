#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const pkgRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const crateDir = join(pkgRoot, "../../iris.rs/iris-wasm");
const stagingDir = join(pkgRoot, "../iris-unknown-wasm32/pkg-staging");
const release = process.argv.includes("--release");

const args = [
    "build",
    ...(release ? ["--release"] : []),
    "--target",
    "web",
    "--out-name",
    "iris.unknown-wasm32",
    "--out-dir",
    stagingDir,
    crateDir,
];

const run = spawnSync("wasm-pack", args, { stdio: "inherit", cwd: pkgRoot });
if (run.status !== 0) {
    process.exit(run.status ?? 1);
}

const copy = spawnSync(process.execPath, [join(pkgRoot, "scripts", "copy-artifacts.mjs")], {
    stdio: "inherit",
    cwd: pkgRoot,
});
process.exit(copy.status ?? 1);
