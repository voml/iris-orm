#!/usr/bin/env node
/**
 * Rename wasm-pack outputs to stable platform filenames and patch the glue import.
 */
import { copyFileSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const staging = join(root, "iris-unknown-wasm32", "pkg-staging");
const out = join(root, "iris-unknown-wasm32");

const wasmSrc = join(staging, "iris.unknown-wasm32_bg.wasm");
const jsSrc = join(staging, "iris.unknown-wasm32.js");
const dtsSrc = join(staging, "iris.unknown-wasm32.d.ts");

const wasmDest = join(out, "iris.unknown-wasm32.wasm");
const jsDest = join(out, "iris.unknown-wasm32.js");
const dtsDest = join(out, "iris.unknown-wasm32.d.ts");

copyFileSync(wasmSrc, wasmDest);

let glue = readFileSync(jsSrc, "utf8");
glue = glue.replaceAll("iris.unknown-wasm32_bg.wasm", "iris.unknown-wasm32.wasm");
writeFileSync(jsDest, glue);

copyFileSync(dtsSrc, dtsDest);

rmSync(staging, { recursive: true, force: true });

console.log(`copied wasm -> ${wasmDest}`);
console.log(`copied glue -> ${jsDest}`);
