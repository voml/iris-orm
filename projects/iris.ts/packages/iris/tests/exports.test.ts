import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { srcImport } from "./helpers.ts";

const pkgRoot = new URL("..", import.meta.url);
const pkg = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));

function resolveExportMap(subpath: string, conditions: string[]) {
    const entry = pkg.exports[subpath];
    if (!entry || typeof entry === "string") {
        throw new Error(`missing exports entry ${subpath}`);
    }
    for (const cond of conditions) {
        if (entry[cond]) {
            return fileURLToPath(new URL(entry[cond], pkgRoot));
        }
    }
    if (entry.default) {
        return fileURLToPath(new URL(entry.default, pkgRoot));
    }
    throw new Error(`unresolved ${subpath} for [${conditions.join(", ")}]`);
}

function facadePath(segment: string, file = "index") {
    return new RegExp(`/(src|dist)/${segment}/${file}\\.(ts|js)$`);
}

test("default export resolves to browser facade", () => {
    const resolved = import.meta.resolve("@yydb/iris", new URL("../package.json", import.meta.url).href);
    assert.match(fileURLToPath(resolved).replace(/\\/g, "/"), facadePath("browser"));
});

test("/node export resolves to node facade on Node", () => {
    const path = resolveExportMap("./node", ["node", "import"]);
    assert.match(path.replace(/\\/g, "/"), facadePath("node"));
});

test("/node default resolves to unsupported stub for browser graphs", () => {
    const path = resolveExportMap("./node", ["browser", "import"]);
    assert.match(path.replace(/\\/g, "/"), facadePath("node", "unsupported"));
});

test("/types export resolves to protocol-only surface", () => {
    const path = resolveExportMap("./types", ["browser", "import"]);
    assert.match(path.replace(/\\/g, "/"), /\/(src|dist)\/types\//);
});

test("/runtime export resolves to runtime helpers", () => {
    const path = resolveExportMap("./runtime", ["browser", "import"]);
    assert.match(path.replace(/\\/g, "/"), facadePath("runtime"));
});

test("/cloudflare export is gone (D1 lowering lives in Rust/WASM, TS only bridges)", () => {
    assert.equal(pkg.exports["./cloudflare"], undefined);
    assert.equal(pkg.typesVersions["*"].cloudflare, undefined);
});

test("@yydb/iris exposes runtime helpers and no WASM entry", async () => {
    const runtime = await import(srcImport("src/runtime/index.ts"));
    assert.equal("buildRuntime" in runtime, true);
    assert.equal("openBindingSession" in runtime, true);
    assert.equal("createIrisDbBindingFromSession" in runtime, true);
    assert.equal("createOperationExecutorFromSession" in runtime, true);
    assert.equal("guardOperationExecutor" in runtime, true);

    assert.equal(pkg.exports["./wasm"], undefined);
    assert.equal(pkg.typesVersions["*"].wasm, undefined);
    assert.equal(pkg.dependencies["@yydb/iris-unknown-wasm32"], undefined);
    // Worker hosts route through @yydb/iris-language which owns the WASM dep.
    assert.equal(pkg.dependencies["@yydb/iris-language"], "workspace:*");
});

test("unsupported /node stub throws node-host-required", async () => {
    const unsupported = await import(srcImport("src/node/unsupported.ts"));
    assert.throws(
        () => unsupported.createIris(),
        (error: unknown) => error instanceof Error && "code" in error && error.code === "node-host-required",
    );
});
