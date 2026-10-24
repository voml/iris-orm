#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const pkgRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = join(pkgRoot, "..", "..", "..", "..");
const transpile = join(repoRoot, "scripts", "transpile-src-to-dist.mjs");

const run = spawnSync(process.execPath, [transpile, pkgRoot], {
    cwd: repoRoot,
    stdio: "inherit",
});
if (run.status !== 0) {
    process.exit(run.status ?? 1);
}

// wasm-bindgen glue and the wasm asset live under `src/lib`; keep the same
// relative layout in `dist` so `dist/index.js` resolves `./lib/iris_wasm.js`.
const srcLib = join(pkgRoot, "src", "lib");
const distLib = join(pkgRoot, "dist", "lib");
if (existsSync(srcLib)) {
    mkdirSync(distLib, { recursive: true });
    for (const name of readdirSync(srcLib)) {
        if (! /\.(js|mjs|wasm|d\.ts)$/.test(name)) {
            continue;
        }
        copyFileSync(join(srcLib, name), join(distLib, name));
    }
}
