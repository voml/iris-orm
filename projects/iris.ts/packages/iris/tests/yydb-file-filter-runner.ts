/**
 * Runner: `file` hash-identity filters against a real on-disk database.
 *
 * Executed inside a generated project root by `yydb-file-filter.test.ts`.
 * Writes three rows through the generated client (two share one content
 * digest under different labels), then proves the digest-identity filter
 * semantics on the generated face, the direct executor face and the
 * `createClient({ dsn })` face, across a close-and-reopen of the same file.
 */
import { createHash } from "node:crypto";

import { Client } from "./operations.js";
import { createClient } from "./node.js";
import { buildDeclaredOperationRequest, synthesizeFindMany } from "./_internal/synthesize.js";
import { IRIS_FIELD_WIRE_NAMES, IRIS_SCHEMA_FINGERPRINT, IRIS_VALUE_CARRIERS } from "./metadata.js";

const projectRoot = process.env.IRIS_TEST_PROJECT;
if (!projectRoot) {
    throw new Error("IRIS_TEST_PROJECT is required");
}

const { createIrisOperationExecutor, loadIrisConfig } = await import("@yydb/iris/node");
const projectConfig = await loadIrisConfig(projectRoot);
const datasourcePath = projectConfig.datasources?.default?.path ?? "data/app.yydb";

const executorOptions = {
    profile: "project",
    config: projectRoot,
    wireNamesByEntity: IRIS_FIELD_WIRE_NAMES,
    valueCarriersByEntity: IRIS_VALUE_CARRIERS,
    contractFingerprint: IRIS_SCHEMA_FINGERPRINT,
};

function sha256Hex(buffer) {
    return createHash("sha256").update(buffer).digest("hex");
}

function descriptorFor(content, name, mediaType) {
    const descriptor = {
        digest: sha256Hex(content),
        octets: BigInt(content.length),
        name,
        manifest: { hash: sha256Hex(Buffer.concat([content, Buffer.from("manifest")])), size: BigInt(content.length) },
    };
    if (mediaType !== undefined) {
        descriptor.mediaType = mediaType;
    }
    return descriptor;
}

const alphaContent = Buffer.from(Array.from({ length: 700 }, (_, index) => index % 256));
const betaContent = Buffer.from(Array.from({ length: 64 }, (_, index) => 255 - (index % 256)));

// Two rows share the alpha digest under different labels; the beta row is
// distinct content. The filter probe below carries yet another label set for
// the same alpha digest — labels must never take part in the comparison.
const alpha = descriptorFor(alphaContent, "alpha.bin", "application/octet-stream");
const alphaRenamed = descriptorFor(alphaContent, "alpha-renamed.bin");
const beta = descriptorFor(betaContent, "beta.bin", "application/octet-stream");
const relabeledProbe = descriptorFor(alphaContent, "unrelated-name.dat", "application/x-other");

const idsOf = (rows) => rows.map((row) => row.assetId.toString()).sort();
const payloadFieldsOf = (rows) =>
    rows.map((row) => ({
        assetId: row.assetId.toString(),
        digest: row.payload.digest,
        octets: row.payload.octets.toString(),
        name: row.payload.name ?? null,
        mediaType: row.payload.mediaType ?? null,
        manifestHash: row.payload.manifest.hash,
        manifestSize: row.payload.manifest.size.toString(),
    }));

const executor = await createIrisOperationExecutor(executorOptions);
const client = new Client(executor);

// Writes go through the generated face with the typed descriptor carrier.
await client.tables.asset.create({ data: { assetId: 1n, label: "alpha", payload: alpha } });
await client.tables.asset.create({ data: { assetId: 2n, label: "alpha-copy", payload: alphaRenamed } });
await client.tables.asset.create({ data: { assetId: 3n, label: "beta", payload: beta } });

// Generated face: equality and inequality on the content digest.
const eqRows = await client.tables.asset.findMany({ where: { payload: { eq: alpha } } });
const notRows = await client.tables.asset.findMany({ where: { payload: { not: alpha } } });
const relabeledRows = await client.tables.asset.findMany({ where: { payload: { eq: relabeledProbe } } });
const allRows = await client.tables.asset.findMany();

// Direct face: the same synthesized operation through the raw executor.
const directEnvelope = await executor.execute(
    buildDeclaredOperationRequest("Asset.findMany", synthesizeFindMany("Asset", { where: { payload: { eq: alpha } } })),
);

// Error path: a descriptor outside the canonical digest form is refused.
let malformedCode = null;
try {
    await client.tables.asset.findMany({ where: { payload: { eq: { ...alpha, digest: "not-a-digest" } } } });
} catch (error) {
    malformedCode = error && typeof error === "object" && "code" in error ? String(error.code) : "unknown";
}

await executor.close();

// Reopen: a fresh session over the same file sees the same identity semantics.
const reopened = await createIrisOperationExecutor(executorOptions);
const reopenedClient = new Client(reopened);
const reopenEqRows = await reopenedClient.tables.asset.findMany({ where: { payload: { eq: alpha } } });
const reopenAllRows = await reopenedClient.tables.asset.findMany();
await reopened.close();

// `createClient({ dsn })` face on the same on-disk file.
const database = createClient({ dsn: `file:${datasourcePath}` });
const databaseFaceRows = await database.tables.asset.findMany({ where: { payload: { eq: alpha } } });

if (!directEnvelope.ok) {
    throw new Error(directEnvelope.diagnostics[0]?.message ?? "direct findMany failed");
}

console.log(
    JSON.stringify({
        ok: true,
        eqIds: idsOf(eqRows),
        notIds: idsOf(notRows),
        relabeledIds: idsOf(relabeledRows),
        directEqIds: idsOf(directEnvelope.value),
        reopenEqIds: idsOf(reopenEqRows),
        reopenAllCount: reopenAllRows.length,
        databaseFaceEqIds: idsOf(databaseFaceRows),
        payloads: payloadFieldsOf(allRows),
        malformedCode,
    }),
);
