#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const homepageRoot = join(dirname(fileURLToPath(import.meta.url)), "..");

/** Run vmz CLI via node (avoids Windows .cmd + path-with-spaces issues). */
export function runVmz(subcommand, vmzArgs, opts = {}) {
    const cli = join(homepageRoot, "node_modules", "@vmz", "vmz", "bin", "vmz.js");
    if (!existsSync(cli)) {
        console.error("missing @vmz/vmz — run pnpm install in homepage");
        process.exit(1);
    }
    const args = [cli, subcommand, ...vmzArgs];
    return spawnSync(process.execPath, args, {
        cwd: homepageRoot,
        stdio: "inherit",
        env: process.env,
        ...opts,
    });
}
