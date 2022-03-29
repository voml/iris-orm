#!/usr/bin/env node
/** Resolve the built N-API artifact path for the current host (monorepo dev / tests). */
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

/** @type {Record<string, { packageDir: string; fileName: string }>} */
const PLATFORMS = {
    "win32-x64": { packageDir: "iris-win32-x64", fileName: "iris-win32-x64-msvc.node" },
    "linux-x64": { packageDir: "iris-linux-x64", fileName: "iris-linux-x64-gnu.node" },
    "linux-arm64": { packageDir: "iris-linux-arm64", fileName: "iris-linux-arm64-gnu.node" },
    "darwin-x64": { packageDir: "iris-darwin-x64", fileName: "iris-darwin-x64.node" },
    "darwin-arm64": { packageDir: "iris-darwin-arm64", fileName: "iris-darwin-arm64.node" },
};

const key = `${process.platform}-${process.arch}`;
const entry = PLATFORMS[key];
if (!entry) {
    console.error(`iris: no platform package mapped for ${key}`);
    process.exit(1);
}

const artifact = join(root, "projects/packages", entry.packageDir, "lib", entry.fileName);
if (!existsSync(artifact)) {
    console.error(`iris: native artifact missing: ${artifact}`);
    console.error("Run: pnpm build:napi");
    process.exit(1);
}

process.stdout.write(artifact);
