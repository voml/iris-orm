#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const pkgRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = join(pkgRoot, "..", "..", "..");
const transpile = join(repoRoot, "scripts", "transpile-src-to-dist.mjs");

const run = spawnSync(process.execPath, [transpile, pkgRoot], {
    cwd: repoRoot,
    stdio: "inherit",
});

process.exit(run.status ?? 1);
