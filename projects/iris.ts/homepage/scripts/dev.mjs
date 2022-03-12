#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { distDirForTarget, DEFAULT_OUT_TARGET, readDefaultProfile } from "./profile-out-dir.mjs";
import { runVmz } from "./run-vmz.mjs";

const profile = process.env.VMZ_PROFILE || readDefaultProfile();
const target = process.env.VMZ_OUT_TARGET || DEFAULT_OUT_TARGET;
const outDir = distDirForTarget(target);
const homepageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const distAbs = path.join(homepageRoot, outDir);

console.log(`@yydb/iris-homepage dev → ${outDir} (target=${target}, profile=${profile})`);

/** @param {string} reason */
async function runPostbuild(reason) {
    try {
        const mod = await import("./postbuild-site.mjs");
        if (typeof mod.runPostbuild === "function") {
            mod.runPostbuild({ reason });
        }
    } catch (e) {
        console.warn("postbuild-site:", e instanceof Error ? e.message : e);
    }
}

const seed = runVmz("build", [".", "--profile", profile, "--out-dir", outDir]);
if ((seed.status ?? 1) !== 0) {
    process.exit(seed.status ?? 1);
}
await runPostbuild("dev-seed");

let timer = null;
/** @param {string} reason */
function schedulePostbuild(reason) {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => {
        void runPostbuild(reason);
    }, 350);
}

try {
    fs.watch(distAbs, { recursive: true }, (_event, file) => {
        if (!file) return;
        const norm = String(file).replace(/\\/g, "/");
        if (
            norm === "entry-client.js" ||
            norm === "document.manifest.json" ||
            norm.startsWith("d/zh-hans/") ||
            norm.startsWith("d/en-us/") ||
            norm.startsWith("d/index.html")
        ) {
            schedulePostbuild(`watch:${norm}`);
        }
    });
} catch {
    /* watch unsupported */
}

const run = runVmz("dev", [".", "--profile", profile, "--out-dir", outDir]);
process.exit(run.status ?? 1);
