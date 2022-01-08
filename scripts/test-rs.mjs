#!/usr/bin/env node
/**
 * `cargo test --workspace` for projects/iris.rs
 */
import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const rustDir = join(dirname(fileURLToPath(import.meta.url)), "..", "projects", "iris.rs");

execFileSync("cargo", ["test", "--workspace", ...process.argv.slice(2)], {
    cwd: rustDir,
    stdio: "inherit",
    env: process.env,
    shell: process.platform === "win32",
});
