#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { copyFileSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const pkgRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = join(pkgRoot, "..", "..", "..", "..");
const transpile = join(repoRoot, "scripts", "transpile-src-to-dist.mjs");
const distRoot = join(pkgRoot, "dist");

function run(command, args) {
    const result = spawnSync(command, args, { cwd: repoRoot, stdio: "inherit" });
    if (result.status !== 0) {
        process.exit(result.status ?? 1);
    }
}

run(process.execPath, [transpile, pkgRoot]);
run(process.execPath, [require.resolve("typescript/bin/tsc"), "-p", join(pkgRoot, "tsconfig.build.json")]);

function rewriteDeclarationSpecifiers(file) {
    const source = readFileSync(file, "utf8");
    const rewritten = source
        .replace(/(from\s*['"])(\.[^'"]+)\.tsx?(['"])/g, "$1$2.js$3")
        .replace(/(import\s*\(\s*['"])(\.[^'"]+)\.tsx?(['"]\s*\))/g, "$1$2.js$3")
        .replace(/(import\s*['"])(\.[^'"]+)\.tsx?(['"])/g, "$1$2.js$3");
    if (rewritten !== source) {
        writeFileSync(file, rewritten, "utf8");
    }
}

function walk(dir) {
    for (const name of readdirSync(dir)) {
        const full = join(dir, name);
        if (statSync(full).isDirectory()) {
            walk(full);
        } else if (full.endsWith(".d.ts") || full.endsWith(".d.mts")) {
            rewriteDeclarationSpecifiers(full);
        }
    }
}

walk(distRoot);

const srcRoot = join(pkgRoot, "src");

function copyDeclarationSources(dir) {
    for (const name of readdirSync(dir)) {
        const full = join(dir, name);
        if (statSync(full).isDirectory()) {
            copyDeclarationSources(full);
        } else if (full.endsWith(".d.mts")) {
            const outPath = join(distRoot, relative(srcRoot, full));
            mkdirSync(dirname(outPath), { recursive: true });
            copyFileSync(full, outPath);
        }
    }
}

copyDeclarationSources(srcRoot);
console.log(`@yydb/iris declarations ok → ${distRoot}`);
