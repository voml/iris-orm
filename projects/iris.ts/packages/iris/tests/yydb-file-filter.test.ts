import assert from "node:assert/strict";
import { copyFile, mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

const SCHEMA = `
table Asset {
    @@asset_id: i64,
    label: utf8,
    payload: file,
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
    const node = await import(new URL("../src/node/index.ts", import.meta.url).href);
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

test("`file` filters discriminate by content hash on a real database file, through every face", async (t) => {
    const core = await loadCore(t);
    if (!core) {
        return;
    }

    // A real on-disk `.yydx` project: the object-store layout keeps the `file`
    // payload carrier persistent across the close-and-reopen leg.
    const projectRoot = await mkdtemp(join(tmpdir(), "iris-yydb-file-filter-"));
    await mkdir(join(projectRoot, "schemas"), { recursive: true });
    await writeFile(join(projectRoot, "schemas", "app.iris"), `${SCHEMA.trim()}\n`, "utf8");
    await writeFile(
        join(projectRoot, "iris.config.ts"),
        `export default {
  schema: "schemas/app.iris",
  datasources: {
    default: { kind: "yydb", mode: "native_pull", path: "data/app.yydx" },
  },
  generate: { out: ".", target: "typescript" },
};`,
        "utf8",
    );

    const generatedRoot = join(projectRoot, "src", "generated", "iris");
    const result = core.generate(SCHEMA, "typescript", generatedRoot);
    assert.equal(result.ok, true, result.error);

    const runnerTemplate = fileURLToPath(new URL("./yydb-file-filter-runner.ts", import.meta.url));
    await copyFile(runnerTemplate, join(generatedRoot, "yydb-file-filter-runner.ts"));

    const bootstrap = pathToFileURL(fileURLToPath(new URL("./generated-e2e-bootstrap.mjs", import.meta.url))).href;
    const child = spawnSync(
        process.execPath,
        ["--experimental-strip-types", "--import", bootstrap, "src/generated/iris/yydb-file-filter-runner.ts"],
        {
            encoding: "utf8",
            cwd: projectRoot,
            env: { ...process.env, IRIS_TEST_PROJECT: projectRoot },
        },
    );
    assert.equal(child.status, 0, child.stdout + child.stderr);

    const payload = JSON.parse(child.stdout.trim()) as {
        ok: boolean;
        eqIds: string[];
        notIds: string[];
        relabeledIds: string[];
        directEqIds: string[];
        reopenEqIds: string[];
        reopenAllCount: number;
        databaseFaceEqIds: string[];
        payloads: Array<{
            assetId: string;
            digest: string;
            octets: string;
            name: string | null;
            mediaType: string | null;
            manifestHash: string;
            manifestSize: string;
        }>;
        malformedCode: string | null;
    };

    assert.equal(payload.ok, true);
    // Two rows share the alpha digest under different labels: equality is the
    // content hash alone, inequality keeps only the other content.
    assert.deepEqual(payload.eqIds, ["1", "2"]);
    assert.deepEqual(payload.notIds, ["3"]);
    // A probe whose labels match nothing stored still matches by digest.
    assert.deepEqual(payload.relabeledIds, ["1", "2"]);
    // The direct executor face sees the same rows as the generated face.
    assert.deepEqual(payload.directEqIds, ["1", "2"]);
    // A fresh session over the same file keeps the identity semantics.
    assert.deepEqual(payload.reopenEqIds, ["1", "2"]);
    assert.equal(payload.reopenAllCount, 3);
    // The `Database` face agrees with the executor face.
    assert.deepEqual(payload.databaseFaceEqIds, ["1", "2"]);

    // Typed contract: descriptors decode with digest, exact `u64` counts and labels.
    assert.equal(payload.payloads.length, 3);
    const byId = new Map(payload.payloads.map((row) => [row.assetId, row]));
    const alpha = byId.get("1");
    const alphaCopy = byId.get("2");
    const beta = byId.get("3");
    assert.ok(alpha && alphaCopy && beta);
    assert.match(alpha.digest, /^[0-9a-f]{64}$/);
    assert.equal(alphaCopy.digest, alpha.digest, "shared content shares the digest identity");
    assert.notEqual(beta.digest, alpha.digest);
    assert.equal(alpha.octets, "700");
    assert.equal(alpha.name, "alpha.bin");
    assert.equal(alpha.mediaType, "application/octet-stream");
    assert.equal(alphaCopy.name, "alpha-renamed.bin", "labels are stored but never compared");
    assert.equal(alphaCopy.mediaType, null);
    assert.match(alpha.manifestHash, /^[0-9a-f]{64}$/);
    assert.equal(alpha.manifestSize, "700");

    // Error path: a malformed digest is refused as a structured failure.
    assert.ok(payload.malformedCode, "a malformed digest must be refused");
    assert.match(payload.malformedCode!, /^IRIS-/);
});
