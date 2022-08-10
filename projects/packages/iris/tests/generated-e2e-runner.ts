import { createClient } from "./operations.js";
import { Database } from "./node.js";
import { buildDeclaredOperationRequest, synthesizeFindMany } from "./_internal/synthesize.js";
import { IRIS_FIELD_WIRE_NAMES, IRIS_SCHEMA_FINGERPRINT } from "./metadata.js";

function readBoolField(row: Record<string, unknown>, field: string): boolean {
    const value = row[field];
    return value === true || value === 1;
}

const schema = process.env.IRIS_TEST_SCHEMA;
if (!schema) {
    throw new Error("IRIS_TEST_SCHEMA is required");
}

const { createIrisOperationExecutor } = await import("@yydb/iris/node");

const executor = await createIrisOperationExecutor({
    profile: "sqlite",
    sqlitePath: ":memory:",
    schema,
    wireNamesByEntity: IRIS_FIELD_WIRE_NAMES,
    contractFingerprint: IRIS_SCHEMA_FINGERPRINT,
});

const synthesis = synthesizeFindMany("User", {
    where: {
        active: { eq: true },
    },
});
const request = buildDeclaredOperationRequest("User.findMany", synthesis);
const directEnvelope = await executor.execute(request);

const client = createClient(executor);
const clientRows = await client.user.findMany({
    where: {
        active: { eq: true },
    },
});
await executor.close();

const db = await Database.create({
    profile: "sqlite",
    sqlitePath: ":memory:",
    schema,
});

const databaseRows = await db.user.findMany({
    where: {
        active: { eq: true },
    },
});

const created = await db.user.create({
    data: { userId: "e2e-1", userName: "Ada", active: true },
    select: { userId: true, userName: true, active: true },
});

const unique = await db.user.findUnique({
    where: { userId: "e2e-1" },
});

const afterCreate = await db.user.findMany({
    where: { userId: { eq: "e2e-1" } },
});

const updated = await db.user.update({
    where: { userId: "e2e-1" },
    data: { userName: { set: "Grace" } },
});

await db.user.delete({ where: { userId: "e2e-1" } });

const afterDelete = await db.user.findMany({
    where: { userId: { eq: "e2e-1" } },
});

await Database.close();

if (!directEnvelope.ok) {
    throw new Error(directEnvelope.diagnostics[0]?.message ?? "generated findMany failed");
}

const createdRow = created as Record<string, unknown>;
const updatedRow = updated as Record<string, unknown>;

console.log(
    JSON.stringify({
        ok: true,
        fingerprint: IRIS_SCHEMA_FINGERPRINT,
        directCount: directEnvelope.value.length,
        clientCount: clientRows.length,
        databaseCount: databaseRows.length,
        createdUserId: created.userId,
        createdUserName: created.userName,
        createdActive: readBoolField(createdRow, "active"),
        uniqueUserName: unique?.userName,
        afterCreateCount: afterCreate.length,
        updatedUserName: updated.userName,
        afterDeleteCount: afterDelete.length,
    }),
);
