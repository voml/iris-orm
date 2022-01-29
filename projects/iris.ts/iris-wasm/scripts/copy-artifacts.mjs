#!/usr/bin/env node
/**
 * Rename wasm-pack outputs to stable platform filenames and patch the glue import.
 */
import { copyFileSync, existsSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const staging = join(root, "iris-unknown-wasm32", "pkg-staging");
const out = join(root, "iris-unknown-wasm32");

const wasmDest = join(out, "iris.unknown-wasm32.wasm");
const jsDest = join(out, "iris.unknown-wasm32.js");
const dtsDest = join(out, "iris.unknown-wasm32.d.ts");

function pickStagingFile(candidates, fallbackExt) {
    for (const name of candidates) {
        const path = join(staging, name);
        if (existsSync(path)) {
            return path;
        }
    }
    if (fallbackExt) {
        const match = readdirSync(staging).find((name) => name.endsWith(fallbackExt) && !name.endsWith(`.wasm${fallbackExt}`));
        if (match) {
            return join(staging, match);
        }
    }
    throw new Error(`missing staging artifact in ${staging} (wanted one of: ${candidates.join(", ")})`);
}

const wasmSrc = pickStagingFile(["iris.unknown-wasm32_bg.wasm", "iris.wasm"], ".wasm");
const jsSrc = pickStagingFile(["iris.unknown-wasm32.js", "iris.js"], ".js");
const dtsSrc = pickStagingFile(["iris.unknown-wasm32.d.ts", "iris.d.ts"], ".d.ts");

copyFileSync(wasmSrc, wasmDest);

let glue = readFileSync(jsSrc, "utf8");
for (const oldName of ["iris.unknown-wasm32_bg.wasm", "iris.wasm", "iris_bg.wasm"]) {
    glue = glue.replaceAll(oldName, "iris.unknown-wasm32.wasm");
}
writeFileSync(jsDest, glue);

copyFileSync(dtsSrc, dtsDest);

rmSync(staging, { recursive: true, force: true });

console.log(`copied wasm -> ${wasmDest}`);
console.log(`copied glue -> ${jsDest}`);
