#!/usr/bin/env node
/**
 * Local N-API smoke: load the current host @yydb/iris-* platform package.
 */
import { createRequire } from "node:module";
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const pkgRoot = join(dirname(fileURLToPath(import.meta.url)), "..");

const artifactOut = spawnSync(process.execPath, [join(pkgRoot, "scripts", "resolve-platform-dir.mjs"), "--artifact"], {
    encoding: "utf8",
});
if (artifactOut.status !== 0) {
    console.error(artifactOut.stderr || artifactOut.stdout);
    process.exit(artifactOut.status ?? 1);
}

const bindingPath = process.env.NAPI_RS_NATIVE_LIBRARY_PATH ?? artifactOut.stdout.trim();
const binding = require(bindingPath);

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
