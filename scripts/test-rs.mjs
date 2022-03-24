#!/usr/bin/env node
/**
 * `cargo test --workspace` for the Rust workspace.
 */
import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = join(dirname(fileURLToPath(import.meta.url)), "..");

execFileSync("cargo", ["test", "--workspace"], {
    cwd: rootDir,
    stdio: "inherit",
    env: process.env,
    shell: process.platform === "win32",
});
