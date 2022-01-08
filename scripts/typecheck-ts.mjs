#!/usr/bin/env node
/**
 * Typecheck all TypeScript workspace packages under projects/iris.ts.
 */
import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = join(dirname(fileURLToPath(import.meta.url)), "..");

execFileSync("pnpm", ["-r", "--filter", "./projects/iris.ts/**", "run", "typecheck"], {
    cwd: rootDir,
    stdio: "inherit",
    env: process.env,
    shell: process.platform === "win32",
});
