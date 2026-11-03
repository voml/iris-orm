/**
 * Runner: `contains` / `startsWith` / `endsWith` filters across every face.
 *
 * Executed inside a generated project root by `string-match-filter.test.ts`.
 * Seeds five text rows through the generated client, then proves the same
 * predicate matrix on the generated face, the direct executor face and the
 * `createClient({ dsn })` face over one on-disk sqlite database.
 */
import { Client } from "./operations.js";
import { createClient } from "./node.js";
import { buildDeclaredOperationRequest, synthesizeFindMany } from "./_internal/synthesize.js";
import { IRIS_FIELD_WIRE_NAMES, IRIS_SCHEMA_FINGERPRINT, IRIS_VALUE_CARRIERS } from "./metadata.js";

const sqlitePath = process.env.IRIS_TEST_SQLITE;
const schema = process.env.IRIS_TEST_SCHEMA ?? "";

const { createIrisOperationExecutor } = await import("@yydb/iris/node");

const executorOptions = {
    profile: "sqlite",
    sqlitePath,
    schema,
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
const client = new Client(executor);

await client.tables.note.create({ data: { id: 1n, title: "alpha" } });
await client.tables.note.create({ data: { id: 2n, title: "beta" } });
await client.tables.note.create({ data: { id: 3n, title: "alphabet" } });
await client.tables.note.create({ data: { id: 4n, title: "" } });
await client.tables.note.create({ data: { id: 5n, title: "hello 世界" } });

const generated = {};
const direct = {};
for (const probe of PROBES) {
    generated[probe.name] = idsOf(await client.tables.note.findMany({ where: probe.where }));
    const envelope = await executor.execute(
        buildDeclaredOperationRequest("Note.findMany", synthesizeFindMany("Note", { where: probe.where })),
    );
    if (!envelope.ok) {
        throw new Error(`direct ${probe.name}: ${envelope.diagnostics[0]?.message ?? "failed"}`);
    }
    direct[probe.name] = idsOf(envelope.value);
}

await executor.close();

const database = createClient({ dsn: `sqlite:${sqlitePath}` });
const databaseFace = {};
for (const probe of PROBES) {
    databaseFace[probe.name] = idsOf(await database.tables.note.findMany({ where: probe.where }));
}

console.log(JSON.stringify({ ok: true, generated, direct, database: databaseFace }));
