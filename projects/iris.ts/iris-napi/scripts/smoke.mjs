#!/usr/bin/env node
/**
 * Local N-API smoke: load @yydb/iris-win32-x64 and call irisVersion / checkSource.
 */
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");

const bindingPath = process.env.NAPI_RS_NATIVE_LIBRARY_PATH;
const binding = bindingPath
    ? require(bindingPath)
    : require(join(root, "iris-win32-x64", "iris.win32-x64-msvc.node"));

const version = binding.irisVersion();
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

const result = binding.checkSource(schema);
console.log("check_source:", JSON.stringify(result));
if (!result.ok) {
    throw new Error(`check_source failed: ${result.error ?? "unknown"}`);
}
if (result.tableCount !== 1) {
    throw new Error(`expected 1 table, got ${result.tableCount}`);
}

console.log("napi smoke: ok");
