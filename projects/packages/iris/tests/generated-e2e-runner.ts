import { createClient } from "./operations.js";
import { Database } from "./node.js";
import { buildDeclaredOperationRequest, synthesizeFindMany } from "./_internal/synthesize.js";
import { IRIS_SCHEMA_FINGERPRINT } from "./metadata.js";

const schema = process.env.IRIS_TEST_SCHEMA;
if (!schema) {
    throw new Error("IRIS_TEST_SCHEMA is required");
}

const { createIrisOperationExecutor } = await import("@yydb/iris/node");

const executor = await createIrisOperationExecutor({
    profile: "sqlite",
    sqlitePath: ":memory:",
    schema,
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
await Database.close();

if (!directEnvelope.ok) {
    throw new Error(directEnvelope.diagnostics[0]?.message ?? "generated findMany failed");
}

console.log(
    JSON.stringify({
        ok: true,
        fingerprint: IRIS_SCHEMA_FINGERPRINT,
        directCount: directEnvelope.value.length,
        clientCount: clientRows.length,
        databaseCount: databaseRows.length,
    }),
);
