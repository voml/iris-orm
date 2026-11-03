import assert from "node:assert/strict";
import { test } from "node:test";

import type { IrisBindings, MemorySessionBinding } from "../src/bindings.ts";
import { openSessionFromDsn } from "../src/node/dsn.ts";
import { openBindingSession, resolveBindingProfile } from "../src/runtime/open-binding-session.ts";
import { IrisFacadeError } from "../src/types/errors.ts";

function stubCore(): { core: IrisBindings; pushed: string[] } {
    const pushed: string[] = [];
    const session: MemorySessionBinding = {
        executeVos: () => ({ ok: true, rowsJson: "[]" }),
        close: () => {},
        managedPush: (schema) => pushed.push(schema),
    };
    const core: IrisBindings = {
        irisVersion: () => "0.0.0-test",
        checkSource: () => ({
            ok: true,
            tableCount: 0,
            schemaFingerprint: "fp",
            generatorVersion: "0.0.0-test",
        }),
        introspectSchema: () => '{"ok":true,"tables":[]}',
        openMemorySession: () => session,
        openSqliteSession: () => session,
        openProjectSession: () => session,
    };
    return { core, pushed };
}

test("resolveBindingProfile infers sqlite and project from options", () => {
    assert.equal(resolveBindingProfile({}), "memory");
    assert.equal(resolveBindingProfile({ sqlitePath: ":memory:" }), "sqlite");
    assert.equal(resolveBindingProfile({ config: "/app" }), "project");
});

test("openBindingSession pushes schema on node memory profile", async () => {
    const { core, pushed } = stubCore();
    await openBindingSession("node", core, { schema: "table User { @@id: uuid }" });
    assert.deepEqual(pushed, ["table User { @@id: uuid }"]);
});

test("openBindingSession rejects non-memory profiles on web host", async () => {
    const { core } = stubCore();
    await assert.rejects(
        () => openBindingSession("web", core, { profile: "sqlite", sqlitePath: ":memory:" }),
        /browser host only supports memory profile/,
    );
});

test("openSessionFromDsn keeps the native semantic kind and diagnostic", () => {
    const { core } = stubCore();
    const envelope = {
        code: "IRIS-SCHEMA-MISMATCH",
        kind: "schema_mismatch",
        message: "MySQL cannot represent the `vector` fields of this schema",
        diagnosticCode: "IRIS-SCHEMA-VECTOR-UNSUPPORTED",
    };
    core.openDsnSession = () => {
        throw new Error(JSON.stringify(envelope));
    };
    assert.throws(
        () => openSessionFromDsn(core, "mysql://iris@127.0.0.1:1", "table Doc { @@id: utf8, v: vector<3> }"),
        (error: unknown) => {
            assert.ok(error instanceof IrisFacadeError);
            assert.equal(error.code, envelope.code);
            assert.equal(error.kind, envelope.kind);
            assert.equal(error.diagnosticCode, envelope.diagnosticCode);
            assert.equal(error.message, envelope.message);
            return true;
        },
    );
});

