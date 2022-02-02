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
    const resolved = import.meta.resolve("@yydb/iris", {
        parentURL: new URL("../package.json", import.meta.url).href,
        conditions: ["node", "import"],
    });
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

test("unsupported /node stub throws node-host-required", async () => {
    const unsupported = await import(srcImport("src/node/unsupported.ts"));
    assert.throws(
        () => unsupported.createIris(),
        (error: unknown) => error instanceof Error && "code" in error && error.code === "node-host-required",
    );
});
