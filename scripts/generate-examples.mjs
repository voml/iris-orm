#!/usr/bin/env node
/**
 * Generate TypeScript clients for all `projects/examples/*` via `@yydb/iris` CLI.
 *
 * Reads each project's `iris.config.ts`, loads schema **data** from the declared pointer,
 * and writes `generated/iris/typescript/` (same as `iris generate --config .` in each example).
 */
import { execSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const examplesRoot = join(root, "projects/examples");
const irisCli = join(root, "projects/packages/iris/bin/iris.ts");

const EXAMPLES = ["hono", "express", "fastify", "next", "nuxt", "sveltekit", "astro"];

function ensureNativeArtifact() {
    const resolveScript = join(root, "scripts/resolve-native-artifact.mjs");
    try {
        execSync(`node "${resolveScript}"`, { encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] });
        return;
    } catch {
        const dll = join(root, "target/release/iris_napi.dll");
        const destDir = join(root, "projects/packages/iris-win32-x64/lib");
        const dest = join(destDir, "iris-win32-x64-msvc.node");
        if (process.platform === "win32" && existsSync(dll)) {
            execSync(`node -e "const fs=require('fs');const d='${destDir.replace(/\\/g, "/")}';fs.mkdirSync(d,{recursive:true});fs.copyFileSync('${dll.replace(/\\/g, "/")}','${dest.replace(/\\/g, "/")}')"`, {
                stdio: "inherit",
                shell: true,
            });
            return;
        }
        throw new Error("Native semantic core missing. Run: pnpm run build:napi");
    }
}

ensureNativeArtifact();

for (const name of EXAMPLES) {
    const exampleDir = join(examplesRoot, name);
    console.log(`\n==> ${name}`);
    execSync(`node --experimental-strip-types "${irisCli}" generate --config "${exampleDir}"`, {
        cwd: root,
        stdio: "inherit",
        shell: true,
    });
}

console.log("\nexamples: generate ok");
