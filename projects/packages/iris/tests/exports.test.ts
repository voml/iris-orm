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

test("default export resolves to browser facade", () => {
    const resolved = import.meta.resolve("@yydb/iris", new URL("../package.json", import.meta.url).href);
    assert.match(fileURLToPath(resolved), /[/\\]src[/\\]browser[/\\]/);
});

test("/node export resolves to node facade on Node", () => {
    const path = resolveExportMap("./node", ["node", "import"]);
    assert.match(path.replace(/\\/g, "/"), /\/src\/node\/index\.ts$/);
});

test("/node default resolves to unsupported stub for browser graphs", () => {
    const path = resolveExportMap("./node", ["browser", "import"]);
    assert.match(path.replace(/\\/g, "/"), /\/src\/node\/unsupported\.ts$/);
});

test("/types export resolves to protocol-only surface", () => {
    const path = resolveExportMap("./types", ["browser", "import"]);
    assert.match(path.replace(/\\/g, "/"), /\/src\/types\//);
});

test("/wasm export resolves to wasm facade", () => {
    const path = resolveExportMap("./wasm", ["browser", "import"]);
    assert.match(path.replace(/\\/g, "/"), /\/src\/wasm\/index\.ts$/);
});

test("@yydb/iris/wasm exposes symmetric binding loader", async () => {
    const wasm = await import(srcImport("src/wasm/index.ts"));
    assert.equal("loadIrisWasm" in wasm, true);
    assert.equal("loadIrisWeb" in wasm, true);
    assert.equal("createIrisDbBinding" in wasm, true);
    assert.equal("getWasmSemanticCore" in wasm, true);
});

test("unsupported /node stub throws node-host-required", async () => {
    const unsupported = await import(srcImport("src/node/unsupported.ts"));
    assert.throws(
        () => unsupported.createIris(),
        (error: unknown) => error instanceof Error && "code" in error && error.code === "node-host-required",
    );
});
