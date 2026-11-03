import assert from "node:assert/strict";
import { copyFile, mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

/** Author-facing failure captured from a generated client call. */
type Failure = { code: string; diagnosticCode: string | null };

const SCHEMA = `
table Doc {
    @@doc_id: i64,
    label: utf8,
    embedding: vector<3>,
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

test("vector `findNear` retrieves by explicit metric on a real database file", async (t) => {
    const core = await loadCore(t);
    if (!core) {
        return;
    }

    const projectRoot = await mkdtemp(join(tmpdir(), "iris-yydb-vector-near-"));
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

    const runnerTemplate = fileURLToPath(new URL("./yydb-vector-near-runner.ts", import.meta.url));
    await copyFile(runnerTemplate, join(generatedRoot, "yydb-vector-near-runner.ts"));

    const bootstrap = pathToFileURL(fileURLToPath(new URL("./generated-e2e-bootstrap.mjs", import.meta.url))).href;
    const child = spawnSync(
        process.execPath,
        ["--experimental-strip-types", "--import", bootstrap, "src/generated/iris/yydb-vector-near-runner.ts"],
        {
            encoding: "utf8",
            cwd: projectRoot,
            env: { ...process.env, IRIS_TEST_PROJECT: projectRoot },
        },
    );
    assert.equal(child.status, 0, child.stdout + child.stderr);

    const payload = JSON.parse(child.stdout.trim()) as {
        ok: boolean;
        wrongDimensionCreateCode: string | null;
        cosineStrongIds: string[];
        cosineAllIds: string[];
        dotStrongIds: string[];
        euclideanCloseIds: string[];
        cosineTakeOneIds: string[];
        cosineFilteredIds: string[];
        directIds: string[];
        dotBoundaryIds: string[];
        euclideanBoundaryIds: string[];
        dimensionFailure: Failure | null;
        metricFailure: Failure | null;
        fieldFailure: Failure | null;
        zeroNormQueryFailure: Failure | null;
        zeroNormRowFailure: Failure | null;
        reopenCosineStrongIds: string[];
    };

    assert.equal(payload.ok, true);
    // Writes keep the declared dimension honest before any row lands.
    assert.ok(payload.wrongDimensionCreateCode, "a wrong-dimension write must be refused");

    // Cosine scores: parallel rows score 1, orthogonal 0, opposite -1.
    assert.deepEqual(payload.cosineStrongIds, ["1", "3"], "cosine >= 0.9 keeps the two parallel rows");
    assert.deepEqual(payload.cosineAllIds, ["1", "3", "2"], "cosine >= 0 keeps the orthogonal row last, closest-first");
    // Dot product is an unbounded score: only the longer parallel row passes 1.5.
    assert.deepEqual(payload.dotStrongIds, ["3"]);
    // Euclidean is a distance: at-or-below threshold, ascending order.
    assert.deepEqual(payload.euclideanCloseIds, ["1", "3", "2"]);
    // Closest-first feeds `take`.
    assert.deepEqual(payload.cosineTakeOneIds, ["1"]);
    // The `where` pre-filter narrows the ranked rows.
    assert.deepEqual(payload.cosineFilteredIds, ["3"]);
    // The direct face composes the same building blocks to the same rows.
    assert.deepEqual(payload.directIds, ["1", "3"]);

    // Inclusive thresholds at the exact boundary, matching the reference semantics.
    assert.deepEqual(payload.dotBoundaryIds, ["2"], "dot score 11 meets threshold 11 inclusively");
    assert.deepEqual(payload.euclideanBoundaryIds, ["1"], "distance 5 meets threshold 5 inclusively, 5.00001 fails");

    // Error paths: every rejection carries the author-facing code plus the
    // generation-time diagnostic that names the exact rejected argument.
    assert.deepEqual(payload.dimensionFailure, {
        code: "IRIS-VECTOR-DIMENSION",
        diagnosticCode: "IRIS-VECTOR-NEAR-DIMENSION",
    });
    assert.deepEqual(payload.metricFailure, {
        code: "IRIS-INVALID-ARGUMENT",
        diagnosticCode: "IRIS-VECTOR-NEAR-METRIC",
    });
    assert.deepEqual(payload.fieldFailure, {
        code: "IRIS-INVALID-ARGUMENT",
        diagnosticCode: "IRIS-VECTOR-NEAR-FIELD",
    });
    assert.deepEqual(payload.zeroNormQueryFailure, {
        code: "IRIS-INVALID-ARGUMENT",
        diagnosticCode: "IRIS-VECTOR-NEAR-ZERO-NORM",
    });
    assert.deepEqual(payload.zeroNormRowFailure, {
        code: "IRIS-INVALID-ARGUMENT",
        diagnosticCode: "IRIS-VECTOR-NEAR-ZERO-NORM",
    });

    // A fresh session over the same file ranks the same rows.
    assert.deepEqual(payload.reopenCosineStrongIds, ["1", "3"]);
});
