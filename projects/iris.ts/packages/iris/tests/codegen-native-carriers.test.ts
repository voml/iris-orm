import assert from "node:assert/strict";
import { mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { srcImport } from "./helpers.ts";

/** Declared operations carry the two native-only VOS labels through generation. */
const OPERATION_SCHEMA = `
table Doc {
    @@doc_id: uuid,
    caption: utf8,
}

query search(embedding: vector) -> vector
query download(doc: uuid) -> file
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

async function generateInto(core: { generate: (schema: string, target: string, root: string) => any }, schema: string, label: string) {
    const outDir = await mkdtemp(join(tmpdir(), `iris-codegen-native-${label}-`));
    const root = join(outDir, "generated", "iris");
    return { root, result: core.generate(schema, "typescript", root) };
}

test("declared `vector` and `file` operations generate native carriers in `operations.ts`", async (t) => {
    const core = await loadCore(t);
    if (!core) {
        return;
    }

    const { root, result } = await generateInto(core, OPERATION_SCHEMA, "ops");
    assert.equal(result.ok, true, result.error);
    assert.equal(result.files.length, 12);

    const operations = await readFile(join(root, "operations.ts"), "utf8");
    assert.match(operations, /export class GeneratedOperations/);
    assert.match(operations, /async search\(embedding: Float32Array\): Promise<Float32Array>/);
    assert.match(
        operations,
        /async download\(doc: string\): Promise<\{ readonly digest: string; readonly octets: bigint; readonly name\?: string; readonly mediaType\?: string; readonly manifest: \{ readonly hash: string; readonly size: bigint \} \}>/,
    );

    // Each operation binds through its durable `op_<hex>` identity, not a guessed name.
    for (const name of ["search", "download"]) {
        const declaredId = operations.match(new RegExp(`\`${name}\` \\(op_id: (op_[0-9a-f]{64})\\)`))?.[1];
        assert.ok(declaredId, `${name} must document a durable op id`);
        assert.ok(
            operations.includes(`buildDeclaredOperationRequest("${declaredId}", synthesis)`),
            `${name} must call through its documented op id`,
        );
    }
    assert.match(operations, /synthesizeOperationCall\("search", \[embedding\], \[false\]\)/);
    assert.match(operations, /synthesizeOperationCall\("download", \[doc\], \[false\]\)/);

    // Table fields keep their scalar carriers; operations never leak into them.
    const metadata = await readFile(join(root, "metadata.ts"), "utf8");
    assert.match(metadata, /IRIS_VALUE_CARRIERS/);
    assert.match(metadata, /docId: "uuid", caption: "utf8"/);
    assert.match(metadata, /IRIS_SCHEMA_FINGERPRINT = "[0-9a-f]{64}"/);
});

test("generation is deterministic for the same declared `vector` and `file` operations", async (t) => {
    const core = await loadCore(t);
    if (!core) {
        return;
    }

    const first = await generateInto(core, OPERATION_SCHEMA, "det-a");
    const second = await generateInto(core, OPERATION_SCHEMA, "det-b");
    assert.equal(first.result.ok, true, first.result.error);
    assert.equal(second.result.ok, true, second.result.error);

    for (const name of ["operations.ts", "metadata.ts"]) {
        const a = await readFile(join(first.root, name), "utf8");
        const b = await readFile(join(second.root, name), "utf8");
        assert.equal(a, b, `${name} must not drift between runs`);
    }
});

test("`vector` fields generate a near surface without an equality filter while `file` fields filter by content hash", async (t) => {
    const core = await loadCore(t);
    if (!core) {
        return;
    }

    // Vector comparison is similarity under an explicit query-side metric,
    // not element-wise equality, so the where surface stays closed while the
    // entity gains `findNear` with the metric named by the caller.
    const vectorField = `table Doc {\n    @@doc_id: uuid,\n    embedding: vector,\n}\n`;
    const vectorRoot = join(await mkdtemp(join(tmpdir(), "iris-codegen-native-vec-")), "generated", "iris");
    const vectorResult = core.generate(vectorField, "typescript", vectorRoot);
    assert.equal(vectorResult.ok, true, vectorResult.error);
    const vectorInputs = await readFile(join(vectorRoot, "inputs.ts"), "utf8");
    assert.match(vectorInputs, /export type SimilarityMetric/);
    assert.match(vectorInputs, /export type DocFindNearArgs/);
    const whereBlock = vectorInputs.slice(vectorInputs.indexOf("export type DocWhereInput"), vectorInputs.indexOf("export type DocCreateInput"));
    assert.equal(whereBlock.includes("embedding"), false, "the vector field must stay out of the where surface");
    const vectorOperations = await readFile(join(vectorRoot, "operations.ts"), "utf8");
    assert.match(vectorOperations, /async findNear\(args: DocFindNearArgs\)/);

    // File equality is content-hash identity: the where surface exposes exactly
    // `eq` / `not` over the file descriptor carrier.
    const fileField = `table Doc {\n    @@doc_id: uuid,\n    attachment: file,\n}\n`;
    const fileRoot = join(await mkdtemp(join(tmpdir(), "iris-codegen-native-file-")), "generated", "iris");
    const fileResult = core.generate(fileField, "typescript", fileRoot);
    assert.equal(fileResult.ok, true, fileResult.error);
    const inputs = await readFile(join(fileRoot, "inputs.ts"), "utf8");
    assert.match(inputs, /export type FileFilter/);
    assert.match(inputs, /attachment\?: .* \| FileFilter/);
});

test("dimensioned `vector<4>` and unknown labels are rejected on operation signatures", async (t) => {
    const core = await loadCore(t);
    if (!core) {
        return;
    }

    const table = `table Doc {\n    @@doc_id: uuid,\n}\n`;
    const dimensioned = core.generate(`${table}query search(embedding: vector<4>) -> utf8`, "typescript", join(await mkdtemp(join(tmpdir(), "iris-codegen-native-dim-")), "generated", "iris"));
    assert.equal(dimensioned.ok, false);
    assert.match(dimensioned.error, /operation `search` uses unsupported type `vector<4>`/);
    assert.match(dimensioned.error, /no declared-vos operation carrier/);

    const unknownField = core.generate(`${table.replace("}\n", "    thing: blob,\n}\n")}`, "typescript", join(await mkdtemp(join(tmpdir(), "iris-codegen-native-unknown-")), "generated", "iris"));
    assert.equal(unknownField.ok, false);
    assert.match(unknownField.error, /unknown VOS label `blob` is not in the carrier table/);

    const unknownOp = core.generate(`${table}query search(embedding: blob) -> utf8`, "typescript", join(await mkdtemp(join(tmpdir(), "iris-codegen-native-unknown-op-")), "generated", "iris"));
    assert.equal(unknownOp.ok, false);
    assert.match(unknownOp.error, /unknown VOS label `blob` is not in the carrier table/);
});
