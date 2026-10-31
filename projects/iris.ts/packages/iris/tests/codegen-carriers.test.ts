import assert from "node:assert/strict";
import { mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { srcImport } from "./helpers.ts";

/** `i64` and `bytes` are the two carriers the JSON wire cannot carry natively. */
const CARRIER_SCHEMA = `
table Metric {
    @@metric_id: uuid,
    seq: i64,
    payload: bytes,
    note: utf8,
}
`;

function ensureNativeOverride(): void {
    if (process.env.NAPI_RS_NATIVE_LIBRARY_PATH) {
        return;
    }
    const resolveScript = fileURLToPath(new URL("../../../../scripts/resolve-native-artifact.mjs", import.meta.url));
    const artifactOut = spawnSync(process.execPath, [resolveScript], {
        encoding: "utf8",
    });
    if (artifactOut.status === 0 && artifactOut.stdout.trim()) {
        process.env.NAPI_RS_NATIVE_LIBRARY_PATH = artifactOut.stdout.trim();
    }
}

async function loadCore(t: { skip: (msg?: string) => void }) {
    ensureNativeOverride();
    const node = await import(srcImport("src/node/index.ts"));
    try {
        return node.loadIrisNative();
    } catch (error) {
        if (error instanceof Error && "code" in error && error.code === "native-package-missing") {
            t.skip("Node semantic core not installed");
            return null;
        }
        throw error;
    }
}

test("generated client carries `i64` and `bytes` through generation", async (t) => {
    const core = await loadCore(t);
    if (!core) {
        return;
    }

    const outDir = await mkdtemp(join(tmpdir(), "iris-codegen-carriers-"));
    const root = join(outDir, "generated", "iris");
    const result = core.generate(CARRIER_SCHEMA, "typescript", root);
    assert.equal(result.ok, true);

    const models = await readFile(join(root, "models.ts"), "utf8");
    assert.match(models, /seq: bigint/);
    assert.match(models, /payload: Uint8Array/);

    const inputs = await readFile(join(root, "inputs.ts"), "utf8");
    assert.match(inputs, /export type BigIntFilter/);
    assert.match(inputs, /export type BytesFilter/);
    assert.match(inputs, /seq\?: bigint \| BigIntFilter/);
    assert.match(inputs, /payload\?: Uint8Array \| BytesFilter/);

    const metadata = await readFile(join(root, "metadata.ts"), "utf8");
    assert.match(metadata, /seq: "i64"/);
    assert.match(metadata, /payload: "bytes"/);
    assert.match(metadata, /IRIS_VALUE_CARRIERS/);
});
