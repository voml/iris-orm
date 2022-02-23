#!/usr/bin/env node
/**
 * Homepage-side check: browser VMZ app must not resolve @yydb/iris/node into the client graph.
 */
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const homepageRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const irisPkgRoot = join(homepageRoot, "..", "iris");
const irisPkg = JSON.parse(readFileSync(join(irisPkgRoot, "package.json"), "utf8"));

function resolveExportMap(subpath, conditions) {
    const entry = irisPkg.exports[subpath];
    if (!entry || typeof entry === "string") {
        throw new Error(`missing exports entry ${subpath}`);
    }
    for (const cond of conditions) {
        if (entry[cond]) {
            return join(irisPkgRoot, entry[cond].replace(/^\.\//, "")).replace(/\\/g, "/");
        }
    }
    if (entry.default) {
        return join(irisPkgRoot, entry.default.replace(/^\.\//, "")).replace(/\\/g, "/");
    }
    throw new Error(`unresolved ${subpath}`);
}

const webBrowser = resolveExportMap(".", ["browser", "import"]);
const nodeBrowser = resolveExportMap("./node", ["browser", "import"]);

console.log("homepage host-export probe (browser export map)\n");
console.log(`  @yydb/iris        → ${webBrowser}`);
console.log(`  @yydb/iris/node   → ${nodeBrowser}`);

let failed = false;
if (!webBrowser.includes("/src/browser/")) {
    console.error("FAIL: default entry must stay on browser facade under browser conditions");
    failed = true;
}
if (!nodeBrowser.includes("/src/node/unsupported")) {
    console.error("FAIL: /node must resolve to unsupported.ts under browser conditions");
    failed = true;
}
if (webBrowser.includes("/src/node/")) {
    console.error("FAIL: default entry pulled node facade into browser graph");
    failed = true;
}

if (failed) {
    process.exit(1);
}

console.log("\nok: homepage bundler graph would not load N-API from default entry\n");

const verify = join(irisPkgRoot, "scripts", "verify-exports.mjs");
if (!existsSync(verify)) {
    console.warn("skip: iris/scripts/verify-exports.mjs not present (homepage browser export probe passed)");
    process.exit(0);
}
const run = spawnSync(process.execPath, ["--experimental-strip-types", verify], {
    stdio: "inherit",
});
process.exit(run.status ?? 1);
