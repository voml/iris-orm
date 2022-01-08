#!/usr/bin/env node
/**
 * Format the iris-orm mono:
 *   - Rust: `cargo fmt` in projects/iris.rs
 *   - JSON / JS / TS / MJS: Biome
 *
 *   node scripts/format.mjs           # write
 *   node scripts/format.mjs --check   # check only
 */
import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = join(dirname(fileURLToPath(import.meta.url)), "..");
const checkOnly = process.argv.includes("--check");
const rustDir = join(rootDir, "projects", "iris.rs");

/**
 * @param {string} file
 * @param {string[]} args
 * @param {string} [cwd]
 */
function runFile(file, args, cwd = rootDir) {
    console.log(`$ ${file} ${args.join(" ")}`);
    execFileSync(file, args, {
        cwd,
        stdio: "inherit",
        env: process.env,
        shell: process.platform === "win32",
    });
}

console.log(checkOnly ? "=== Format check ===\n" : "=== Format (write) ===\n");

console.log("--- Rust (projects/iris.rs) ---");
runFile("cargo", ["fmt", "--all", ...(checkOnly ? ["--", "--check"] : [])], rustDir);

console.log("\n--- Biome (json/js/ts/mjs) ---");
if (checkOnly) {
    runFile("pnpm", ["exec", "biome", "ci", "--formatter-enabled=true", "--linter-enabled=false", "--assist-enabled=false", "."]);
} else {
    runFile("pnpm", ["exec", "biome", "format", "--write", "."]);
}

console.log(checkOnly ? "\nFormat check OK." : "\nFormat OK.");
