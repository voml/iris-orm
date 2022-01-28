#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const pkgRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const resolveScript = join(pkgRoot, "scripts", "resolve-platform-dir.mjs");

const out = spawnSync(process.execPath, [resolveScript], { encoding: "utf8" });
if (out.status !== 0) {
    process.stderr.write(out.stderr);
    process.exit(out.status ?? 1);
}
const outputDir = out.stdout.trim();
const release = process.argv.includes("--release");
const args = [
    "build",
    "--platform",
    ...(release ? ["--release"] : []),
    "--manifest-path",
    "../../iris.rs/Cargo.toml",
    "--package",
    "iris-napi",
    "--output-dir",
    outputDir,
];

const run = spawnSync("napi", args, { stdio: "inherit", shell: true, cwd: pkgRoot });
process.exit(run.status ?? 1);
