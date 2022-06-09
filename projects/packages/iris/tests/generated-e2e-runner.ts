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
const envelope = await executor.execute(request);
await executor.close();

if (!envelope.ok) {
    throw new Error(envelope.diagnostics[0]?.message ?? "generated findMany failed");
}

console.log(
    JSON.stringify({
        ok: true,
        fingerprint: IRIS_SCHEMA_FINGERPRINT,
        count: envelope.value.length,
    }),
);
