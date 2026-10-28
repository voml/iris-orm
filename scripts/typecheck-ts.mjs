#!/usr/bin/env node
/**
 * Typecheck all TypeScript workspace packages under projects/iris.ts.
 */
import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = join(dirname(fileURLToPath(import.meta.url)), "..");

function runPnpm(args) {
    execFileSync("pnpm", args, {
        cwd: rootDir,
        stdio: "inherit",
        env: process.env,
        shell: process.platform === "win32",
    });
}

// `@yydb/iris` resolves `@yydb/iris-language/cloudflare/d1-wasm-bridge` through the
// published `exports` of that package, which point at `dist`. Emit `dist` first so
// typecheck does not depend on an earlier build step happening to have run.
runPnpm(["--filter", "@yydb/iris-language", "run", "build"]);

runPnpm(["-r", "--filter", "./projects/iris.ts/packages/**", "--filter", "./projects/iris.ts/platforms/**", "--filter", "./projects/iris.ts/homepage", "run", "typecheck"]);
