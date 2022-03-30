#!/usr/bin/env node
/** Resolve the optional @yydb/iris-* platform package directory for the current host. */
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const pkgRoot = join(dirname(fileURLToPath(import.meta.url)), "..");

/** @type {Record<string, { dir: string; artifact: string }>} */
const PLATFORM_PACKAGES = {
    "win32-x64": {
        dir: join(pkgRoot, "../iris-win32-x64"),
        artifact: "iris.win32-x64-msvc.node",
    },
    "linux-x64": {
        dir: join(pkgRoot, "../iris-linux-x64"),
        artifact: "iris.linux-x64-gnu.node",
    },
};

const key = `${process.platform}-${process.arch}`;
const entry = PLATFORM_PACKAGES[key];

if (!entry) {
    console.error(`@yydb/iris-napi: no platform package mapped for ${key}`);
    process.exit(1);
}

if (process.argv.includes("--artifact")) {
    console.log(join(entry.dir, entry.artifact));
} else {
    console.log(entry.dir);
}
