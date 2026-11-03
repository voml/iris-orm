// Generated TypeScript client against a live yyds SQL gateway.
//
// The DSN is the only input; the client never learns which backend answered.
// Every `code` this prints is compared byte-for-byte with the Python suite.

import assert from "node:assert/strict";

import { createClient } from "./.generated/ts/node.ts";

const dsn = process.env.IRIS_E2E_DSN;
if (!dsn) throw new Error("IRIS_E2E_DSN is required");

const db = createClient({ dsn });
const codes: Record<string, string | null> = {};

function authorCode(error: unknown): string | null {
    if (typeof error !== "object" || error === null) return null;
    const code = (error as { code?: unknown }).code;
    return typeof code === "string" ? code : null;
}

const SEED = [
    { articleId: "a1", slug: "alpha-one", body: "needle in the alpha body" },
    { articleId: "a2", slug: "beta-two", body: "beta body" },
    { articleId: "a3", slug: "gamma-three", body: "gamma needle tail" },
];

for (const row of SEED) {
    await db.tables.article.create({ data: row });
}

// CRUD round-trip.
const unique = await db.tables.article.findUnique({ where: { articleId: "a1" } });
assert.equal(unique?.slug, "alpha-one");

// Text predicates: `contains`, `startsWith`, `endsWith`.
const contains = await db.tables.article.findMany({ where: { body: { contains: "needle" } } });
assert.deepEqual(
    contains.map((row) => row.articleId).sort(),
    ["a1", "a3"],
);
const startsWith = await db.tables.article.findMany({ where: { slug: { startsWith: "alpha" } } });
assert.deepEqual(
    startsWith.map((row) => row.articleId),
    ["a1"],
);
const endsWith = await db.tables.article.findMany({ where: { slug: { endsWith: "-three" } } });
assert.deepEqual(
    endsWith.map((row) => row.articleId),
    ["a3"],
);

// Vector near: only yydb binds typed vector payloads today, so the probe
// records the author code instead of asserting a ranking on this backend.
try {
    await db.tables.doc.create({ data: { docId: "d1", label: "probe", embedding: new Float32Array([1, 0, 0]) } });
    await db.tables.doc.findNear({
        field: "embedding",
        near: { vector: new Float32Array([1, 0, 0]), metric: "cosine", threshold: 0.1 },
    });
    codes.vector_payload = null;
} catch (error) {
    codes.vector_payload = authorCode(error);
}

// Transaction commit.
await db.transaction(async (tx) => {
    await tx.tables.article.create({ data: { articleId: "a4", slug: "delta-four", body: "committed row" } });
});
assert.equal((await db.tables.article.findUnique({ where: { articleId: "a4" } }))?.slug, "delta-four");

// Transaction rollback.
await assert.rejects(
    db.transaction(async (tx) => {
        await tx.tables.article.create({ data: { articleId: "a5", slug: "epsilon-five", body: "rolled back" } });
        throw new Error("rollback probe");
    }),
    /rollback probe/,
);
assert.equal(await db.tables.article.findUnique({ where: { articleId: "a5" } }), null);

// Error path: duplicate slug must keep the author code and carry the backend diagnostic.
const uniqueDiagnostic = process.env.IRIS_E2E_UNIQUE_DIAGNOSTIC;
try {
    await db.tables.article.create({ data: { articleId: "a6", slug: "alpha-one", body: "duplicate" } });
    codes.unique_conflict = null;
} catch (error) {
    const diagnostic = (error as { diagnosticCode?: unknown }).diagnosticCode;
    assert.ok(
        typeof diagnostic === "string" && (uniqueDiagnostic === undefined || diagnostic.includes(uniqueDiagnostic)),
        `unique conflict diagnostic ${String(diagnostic)} must contain ${uniqueDiagnostic}`,
    );
    codes.unique_conflict = authorCode(error);
    codes.unique_conflict_diagnostic = "present";
}

// Error path: `update` on a missing key.
try {
    await db.tables.article.update({ where: { articleId: "missing" }, data: { body: "no match" } });
    codes.not_found = null;
} catch (error) {
    codes.not_found = authorCode(error);
}

console.log(`RESULT ${JSON.stringify({ codes })}`);
