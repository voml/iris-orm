#!/usr/bin/env node
/**
 * Build Iris homepage → dist/<target>/ (default: dist/cdn).
 *
 * VMZ delivery profile (web-static) is the build contract; `cdn` is the
 * upload-facing folder name (any static host / CDN).
 *
 * Usage:
 *   node scripts/build.mjs [--release] [--target cdn] [--profile web-static]
 *   VMZ_OUT_TARGET=cdn VMZ_PROFILE=web-static node scripts/build.mjs --release
 */
import { distDirForTarget, DEFAULT_OUT_TARGET, readDefaultProfile } from "./profile-out-dir.mjs";
import { runVmz } from "./run-vmz.mjs";

function parseArgs(argv) {
    let profile = process.env.VMZ_PROFILE || "";
    let target = process.env.VMZ_OUT_TARGET || "";
    let release = false;
    for (let i = 0; i < argv.length; i++) {
        const a = argv[i];
        if (a === "--profile" && argv[i + 1]) {
            profile = argv[++i];
        } else if (a.startsWith("--profile=")) {
            profile = a.slice("--profile=".length);
        } else if (a === "--target" && argv[i + 1]) {
            target = argv[++i];
        } else if (a.startsWith("--target=")) {
            target = a.slice("--target=".length);
        } else if (a === "--release") {
            release = true;
        }
    }
    if (!profile) profile = readDefaultProfile();
    if (!target) target = DEFAULT_OUT_TARGET;
    return { profile, target, release };
}

const { profile, target, release } = parseArgs(process.argv.slice(2));
const outDir = distDirForTarget(target);

const args = [".", "--profile", profile, "--out-dir", outDir];
if (release) args.push("--release");

console.log(
    `@yydb/iris-homepage build → ${outDir} (target=${target}, profile=${profile}${release ? ", release" : ""})`,
);

const run = runVmz("build", args);
process.exit(run.status ?? 1);
