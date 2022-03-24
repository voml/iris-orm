#!/usr/bin/env node
/**
 * Typecheck all TypeScript workspace packages under projects/packages.
 */
import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = join(dirname(fileURLToPath(import.meta.url)), "..");

execFileSync("pnpm", ["-r", "--filter", "./projects/packages/**", "run", "typecheck"], {
    cwd: rootDir,
    stdio: "inherit",
    env: process.env,
    shell: process.platform === "win32",
});
