#!/usr/bin/env node
/**
 * `cargo check --workspace --all-targets` plus wasm32 release build for iris-wasm.
 */
import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = join(dirname(fileURLToPath(import.meta.url)), "..");

execFileSync("cargo", ["check", "--workspace", "--all-targets"], {
    cwd: rootDir,
    stdio: "inherit",
    env: process.env,
    shell: process.platform === "win32",
});

execFileSync("cargo", ["build", "-p", "iris-wasm", "--target", "wasm32-unknown-unknown", "--release"], {
    cwd: rootDir,
    stdio: "inherit",
    env: process.env,
    shell: process.platform === "win32",
});
