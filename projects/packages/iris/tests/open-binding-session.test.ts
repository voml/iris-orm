import assert from "node:assert/strict";
import { test } from "node:test";

import type { IrisBindings, MemorySessionBinding } from "../src/bindings.ts";
import { openBindingSession, resolveBindingProfile } from "../src/runtime/open-binding-session.ts";

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

test("resolveBindingProfile infers sqlite, opfs, and project from options", () => {
    assert.equal(resolveBindingProfile({}), "memory");
    assert.equal(resolveBindingProfile({ sqlitePath: ":memory:" }), "sqlite");
    assert.equal(resolveBindingProfile({ opfsPath: "app" }), "opfs");
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

test("openBindingSession rejects opfs profile on web host", async () => {
    const { core } = stubCore();
    await assert.rejects(
        () => openBindingSession("web", core, { profile: "opfs", opfsPath: "app" }),
        /@yydb\/iris\/opfs/,
    );
});
