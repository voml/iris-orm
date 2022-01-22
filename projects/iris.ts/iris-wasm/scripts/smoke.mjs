#!/usr/bin/env node
/**
 * Local WASM smoke: init glue + irisVersion / checkSource.
 */
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "iris-unknown-wasm32");
const wasmPath = join(root, "iris.unknown-wasm32.wasm");
const jsPath = join(root, "iris.unknown-wasm32.js");

readFileSync(wasmPath);
const mod = await import(`file://${jsPath.replace(/\\/g, "/")}`);
await mod.default(readFileSync(wasmPath));

const version = mod.irisVersion();
console.log(`iris_version: ${version}`);
if (!version || version === "0.0.0") {
    throw new Error("unexpected iris_version");
}

const schema = `
table User {
    @@user_id: utf8,
    @user_name: utf8,
    active: bool,
}
`;

const result = mod.checkSource(schema);
console.log("check_source:", JSON.stringify({
    ok: result.ok,
    tableCount: result.tableCount,
    schemaFingerprint: result.schemaFingerprint,
    generatorVersion: result.generatorVersion,
    error: result.error ?? null,
}));
if (!result.ok) {
    throw new Error(`check_source failed: ${result.error ?? "unknown"}`);
}
if (result.tableCount !== 1) {
    throw new Error(`expected 1 table, got ${result.tableCount}`);
}

console.log("wasm smoke: ok");
