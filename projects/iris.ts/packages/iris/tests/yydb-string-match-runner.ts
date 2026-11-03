/**
 * Runner: `contains` / `startsWith` / `endsWith` on both YYDB layouts.
 *
 * Executed inside a generated project root by `yydb-layout-string-match.test.ts`.
 * Seeds five text rows plus one `NULL` row through the generated client, then
 * proves the same predicate matrix on the generated face, the direct executor
 * face and the `Database` face. The project config decides whether the layout
 * is a single-file `.yydb` database or a `.yydx` folder database.
 */
import { createClient } from "./operations.js";
import { Database } from "./node.js";
import { buildDeclaredOperationRequest, synthesizeFindMany } from "./_internal/synthesize.js";
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

const PROBES = [
    { name: "contains", where: { title: { contains: "al" } } },
    { name: "startsWith", where: { title: { startsWith: "al" } } },
    { name: "endsWith", where: { title: { endsWith: "ta" } } },
    { name: "endsWithWhole", where: { title: { endsWith: "alpha" } } },
    { name: "containsEmpty", where: { title: { contains: "" } } },
    { name: "startsWithEmpty", where: { title: { startsWith: "" } } },
    { name: "endsWithEmpty", where: { title: { endsWith: "" } } },
    { name: "caseSensitive", where: { title: { contains: "Alpha" } } },
    { name: "needleLongerThanText", where: { title: { startsWith: "alphabetx" } } },
    { name: "unicode", where: { title: { contains: "世界" } } },
];

const idsOf = (rows) =>
    rows.map((row) => row.id.toString()).sort((left, right) => Number(left) - Number(right));

const executor = await createIrisOperationExecutor(executorOptions);
const client = createClient(executor);

await client.note.create({ data: { id: 1n, title: "alpha" } });
await client.note.create({ data: { id: 2n, title: "beta" } });
await client.note.create({ data: { id: 3n, title: "alphabet" } });
await client.note.create({ data: { id: 4n, title: "" } });
await client.note.create({ data: { id: 5n, title: "hello 世界" } });
// A `NULL` text row: every text predicate is unknown for it, so it is never
// part of a filtered result even though it exists in the table.
await client.note.create({ data: { id: 6n, title: null } });

const generated = {};
const direct = {};
for (const probe of PROBES) {
    generated[probe.name] = idsOf(await client.note.findMany({ where: probe.where }));
    const envelope = await executor.execute(
        buildDeclaredOperationRequest("Note.findMany", synthesizeFindMany("Note", { where: probe.where })),
    );
    if (!envelope.ok) {
        throw new Error(`direct ${probe.name}: ${envelope.diagnostics[0]?.message ?? "failed"}`);
    }
    direct[probe.name] = idsOf(envelope.value);
}

const allIds = idsOf(await client.note.findMany());

await executor.close();

const database = await Database.create(executorOptions);
const databaseFace = {};
for (const probe of PROBES) {
    databaseFace[probe.name] = idsOf(await database.note.findMany({ where: probe.where }));
}
await database.$close();

console.log(JSON.stringify({ ok: true, allIds, generated, direct, database: databaseFace }));
