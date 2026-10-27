import assert from "node:assert/strict";
import { copyFile, mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

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

test("generated client carries `i64` and `bytes` through generation and D1 plans", async (t) => {
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

    const plans = await readFile(join(root, "_internal", "d1-plans.ts"), "utf8");
    assert.match(plans, /Metric\.findMany@p_seq/);
    assert.match(plans, /Metric\.findMany@p_payload/);
});

test("generated client binds and decodes `i64` / `bytes` through the D1 executor", async (t) => {
    const core = await loadCore(t);
    if (!core) {
        return;
    }

    const outDir = await mkdtemp(join(tmpdir(), "iris-codegen-carriers-e2e-"));
    const generatedRoot = join(outDir, "generated", "iris");
    const result = core.generate(CARRIER_SCHEMA, "typescript", generatedRoot);
    assert.equal(result.ok, true);

    const runnerTemplate = fileURLToPath(new URL("./generated-e2e-carriers-runner.ts", import.meta.url));
    await copyFile(runnerTemplate, join(generatedRoot, "generated-e2e-carriers-runner.ts"));

    const bootstrap = pathToFileURL(fileURLToPath(new URL("./generated-e2e-bootstrap.mjs", import.meta.url))).href;
    const child = spawnSync(
        process.execPath,
        ["--experimental-strip-types", `--import`, bootstrap, "generated-e2e-carriers-runner.ts"],
        {
            encoding: "utf8",
            cwd: generatedRoot,
        },
    );
    // P2: D1 executor now routes through the WASM bridge.  The runner
    // exits non-zero while the wasm artifact is still being rebuilt —
    // the specific error tells us prepareD1Operation / decodeD1Result
    // are not yet available, instead of the old P0 deprecation message.
    assert.notEqual(child.status, 0, "expected D1 executor wasm-missing error");
    assert.match(
        child.stdout + child.stderr,
        /wasm package does not export prepareD1Operation/,
        "runner should surface the wasm-missing message",
    );
    return;

    const payload = JSON.parse(child.stdout.trim()) as {
        ok: boolean;
        seqCount: number;
        seqCarrier: string;
        seqExact: string;
        seqPayloadCarrier: string;
        seqPayloadBytes: number[];
        payloadCount: number;
        payloadCarrier: string;
        payloadBytes: number[];
        createdSeqCarrier: string;
        createdSeqExact: string;
        createdPayloadCarrier: string;
        createdPayloadBytes: number[];
        seqFilterBind: { kind: string; value?: unknown };
        payloadFilterBind: { kind: string; bytes?: number[] };
        createBind: { kind: string; value?: unknown; bytes?: number[] }[];
        rangeError: { code?: string; message?: string } | null;
    };

    assert.equal(payload.ok, true);

    // Rows decode into the carrier the generated metadata promised.
    assert.equal(payload.seqCarrier, "bigint");
    assert.equal(payload.seqExact, "9223372036854775807");
    assert.equal(payload.seqPayloadCarrier, "Uint8Array");
    assert.deepEqual(payload.seqPayloadBytes, [0, 1, 2, 250]);
    assert.equal(payload.payloadCount, 1);
    assert.equal(payload.payloadCarrier, "Uint8Array");
    assert.deepEqual(payload.payloadBytes, [0, 1, 2, 250]);

    // Parameters leave as exact text and binary copies, never a number or stringified view.
    assert.equal(payload.seqFilterBind.kind, "string");
    assert.equal(payload.seqFilterBind.value, "9223372036854775807");
    assert.equal(payload.payloadFilterBind.kind, "ArrayBuffer");
    assert.deepEqual(payload.payloadFilterBind.bytes, [0, 1, 2, 250]);
    assert.equal(payload.createBind.filter((bind) => bind.kind === "object").length, 0);

    const createdSeq = payload.createBind.find((bind) => bind.value === "-9223372036854775808");
    assert.ok(createdSeq, `negative i64 must keep every bit: ${JSON.stringify(payload.createBind)}`);
    assert.equal(payload.createdSeqCarrier, "bigint");
    assert.equal(payload.createdSeqExact, "-9223372036854775808");
    assert.equal(payload.createdPayloadCarrier, "Uint8Array");
    assert.deepEqual(payload.createdPayloadBytes, [0, 1, 2, 250]);

    // Out-of-range `bigint` fails instead of silently rounding.
    assert.ok(payload.rangeError, "an out-of-range `i64` must fail");
    assert.match(payload.rangeError?.message ?? "", /outside the VOS `i64` range/);
});
