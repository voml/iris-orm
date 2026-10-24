import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";

import { stagePackageFiles } from "./publish-npm.mjs";

function makePackage(files) {
    const root = mkdtempSync(path.join(os.tmpdir(), "iris-publish-test-"));
    writeFileSync(path.join(root, "package.json"), JSON.stringify({ name: "@yydb/test-package", files }));
    writeFileSync(path.join(root, "README.md"), "# test package\n");
    return root;
}

test("missing declared publish path fails fast with package name and path", () => {
    const root = makePackage(["dist", "README.md"]);
    const stage = mkdtempSync(path.join(os.tmpdir(), "iris-publish-stage-"));

    assert.throws(
        () => stagePackageFiles(root, stage),
        (error) =>
            error instanceof Error &&
            error.message.includes("@yydb/test-package") &&
            error.message.includes('"dist"'),
    );
});

test("declared paths that exist are staged", () => {
    const root = makePackage(["dist", "README.md"]);
    mkdirSync(path.join(root, "dist"), { recursive: true });
    writeFileSync(path.join(root, "dist", "index.js"), "export {};\n");
    const stage = mkdtempSync(path.join(os.tmpdir(), "iris-publish-stage-"));

    stagePackageFiles(root, stage);

    assert.ok(existsSync(path.join(stage, "dist", "index.js")));
    assert.ok(existsSync(path.join(stage, "README.md")));
    assert.ok(existsSync(path.join(stage, "package.json")));
});
