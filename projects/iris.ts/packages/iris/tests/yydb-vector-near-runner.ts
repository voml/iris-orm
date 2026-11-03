/**
 * Runner: vector `findNear` similarity retrieval against a real database.
 *
 * Executed inside a generated project root by `yydb-vector-near.test.ts`.
 * Writes `vector<3>` rows through the generated client, then proves the
 * explicit-metric similarity surface on the generated face and the direct
 * executor face: threshold direction per metric, closest-first ordering,
 * `where` pre-filter, `take`, dimension mismatch and zero-norm cosine as
 * structured rejections, and the same semantics after a close-and-reopen.
 */
import { Client } from "./operations.js";
import { buildDeclaredOperationRequest, evaluateVectorNearRows, synthesizeVectorNear } from "./_internal/synthesize.js";
import { IRIS_FIELD_WIRE_NAMES, IRIS_SCHEMA_FINGERPRINT, IRIS_VALUE_CARRIERS } from "./metadata.js";

const projectRoot = process.env.IRIS_TEST_PROJECT;
if (!projectRoot) {
    throw new Error("IRIS_TEST_PROJECT is required");
}

const { createIrisOperationExecutor } = await import("@yydb/iris/node");

const executorOptions = {
    profile: "project",
    config: projectRoot,
    wireNamesByEntity: IRIS_FIELD_WIRE_NAMES,
    valueCarriersByEntity: IRIS_VALUE_CARRIERS,
    contractFingerprint: IRIS_SCHEMA_FINGERPRINT,
};

const idsOf = (rows) => rows.map((row) => row.docId.toString());
const codes = async (fn) => {
    try {
        await fn();
        return null;
    } catch (error) {
        return error && typeof error === "object" && "code" in error ? String(error.code) : "unknown";
    }
};

const executor = await createIrisOperationExecutor(executorOptions);
const client = new Client(executor);

// One row per direction so cosine covers parallel / orthogonal / opposite.
await client.tables.doc.create({ data: { docId: 1n, label: "east", embedding: new Float32Array([1, 0, 0]) } });
await client.tables.doc.create({ data: { docId: 2n, label: "north", embedding: new Float32Array([0, 1, 0]) } });
await client.tables.doc.create({ data: { docId: 3n, label: "east2", embedding: new Float32Array([2, 0, 0]) } });
await client.tables.doc.create({ data: { docId: 4n, label: "west", embedding: new Float32Array([-1, 0, 0]) } });

// A wrong-dimension write is refused before any row lands.
const wrongDimensionCreateCode = await codes(() =>
    client.tables.doc.create({ data: { docId: 99n, label: "bad", embedding: new Float32Array([1, 0]) } }),
);

// Generated face: score metrics pass at or above the threshold.
const cosineStrong = await client.tables.doc.findNear({ field: "embedding", near: { vector: new Float32Array([1, 0, 0]), metric: "cosine", threshold: 0.9 } });
const cosineAll = await client.tables.doc.findNear({ field: "embedding", near: { vector: new Float32Array([1, 0, 0]), metric: "cosine", threshold: 0.0 } });
const dotStrong = await client.tables.doc.findNear({ field: "embedding", near: { vector: new Float32Array([1, 0, 0]), metric: "dot", threshold: 1.5 } });
// Distance metric passes at or below the threshold and orders ascending.
const euclideanClose = await client.tables.doc.findNear({ field: "embedding", near: { vector: new Float32Array([1, 0, 0]), metric: "euclidean", threshold: 1.5 } });
// Closest-first ordering feeds `take`.
const cosineTakeOne = await client.tables.doc.findNear({ field: "embedding", near: { vector: new Float32Array([1, 0, 0]), metric: "cosine", threshold: 0.0 }, take: 1 });
// The `where` pre-filter narrows the scanned rows before ranking.
const cosineFiltered = await client.tables.doc.findNear({
    field: "embedding",
    near: { vector: new Float32Array([1, 0, 0]), metric: "cosine", threshold: 0.0 },
    where: { label: { eq: "east2" } },
});

// Direct face: same building blocks composed by hand must agree.
const directArgs = { field: "embedding", near: { vector: new Float32Array([1, 0, 0]), metric: "cosine", threshold: 0.9 } };
const directEnvelope = await executor.execute(buildDeclaredOperationRequest("Doc.findNear", synthesizeVectorNear("Doc", directArgs)));
if (!directEnvelope.ok) {
    throw new Error(directEnvelope.diagnostics[0]?.message ?? "direct near scan failed");
}
const directRows = evaluateVectorNearRows("Doc", directArgs, directEnvelope.value);

// Inclusive thresholds hold exactly at the boundary, in both directions.
const conformanceRows = [
    { docId: 1n, label: "a", embedding: new Float32Array([1, 2, 0]) },
    { docId: 2n, label: "b", embedding: new Float32Array([3, 4, 0]) },
];
const dotBoundary = evaluateVectorNearRows("Doc", { field: "embedding", near: { vector: new Float32Array([1, 2, 0]), metric: "dot", threshold: 11 } }, conformanceRows);
const euclideanBoundaryRows = [
    { docId: 1n, label: "at", embedding: new Float32Array([3, 4, 0]) },
    { docId: 2n, label: "past", embedding: new Float32Array([3.00001, 4, 0]) },
];
const euclideanBoundary = evaluateVectorNearRows(
    "Doc",
    { field: "embedding", near: { vector: new Float32Array([0, 0, 0]), metric: "euclidean", threshold: 5 } },
    euclideanBoundaryRows,
);

// Error paths: explicit structured rejections, never silent matching.
const dimensionCode = await codes(() =>
    client.tables.doc.findNear({ field: "embedding", near: { vector: new Float32Array([1, 0]), metric: "cosine", threshold: 0.5 } }),
);
const metricCode = await codes(() => client.tables.doc.findNear({ field: "embedding", near: { vector: new Float32Array([1, 0, 0]), metric: "manhattan", threshold: 0.5 } }));
const fieldCode = await codes(() => client.tables.doc.findNear({ field: "label", near: { vector: new Float32Array([1, 0, 0]), metric: "cosine", threshold: 0.5 } }));
const zeroNormQueryCode = await codes(() =>
    client.tables.doc.findNear({ field: "embedding", near: { vector: new Float32Array([0, 0, 0]), metric: "cosine", threshold: 0.0 } }),
);

await executor.close();

// Reopen: a fresh session over the same file ranks the same rows.
const reopened = await createIrisOperationExecutor(executorOptions);
const reopenedClient = new Client(reopened);
const reopenCosineStrong = await reopenedClient.tables.doc.findNear({ field: "embedding", near: { vector: new Float32Array([1, 0, 0]), metric: "cosine", threshold: 0.9 } });
await reopened.close();

// A zero-norm row poisons cosine retrieval explicitly instead of being skipped.
const poisoned = await createIrisOperationExecutor(executorOptions);
const poisonedClient = new Client(poisoned);
await poisonedClient.tables.doc.create({ data: { docId: 5n, label: "zero", embedding: new Float32Array([0, 0, 0]) } });
const zeroNormRowCode = await codes(() =>
    poisonedClient.tables.doc.findNear({ field: "embedding", near: { vector: new Float32Array([1, 0, 0]), metric: "cosine", threshold: 0.0 } }),
);
await poisoned.close();

console.log(
    JSON.stringify({
        ok: true,
        wrongDimensionCreateCode,
        cosineStrongIds: idsOf(cosineStrong),
        cosineAllIds: idsOf(cosineAll),
        dotStrongIds: idsOf(dotStrong),
        euclideanCloseIds: idsOf(euclideanClose),
        cosineTakeOneIds: idsOf(cosineTakeOne),
        cosineFilteredIds: idsOf(cosineFiltered),
        directIds: idsOf(directRows),
        dotBoundaryIds: idsOf(dotBoundary),
        euclideanBoundaryIds: idsOf(euclideanBoundary),
        dimensionCode,
        metricCode,
        fieldCode,
        zeroNormQueryCode,
        zeroNormRowCode,
        reopenCosineStrongIds: idsOf(reopenCosineStrong),
    }),
);
