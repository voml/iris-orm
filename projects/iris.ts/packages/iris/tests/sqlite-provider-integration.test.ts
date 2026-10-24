import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

import { buildOperationRequest, declaredVosOperation } from "../src/runtime/build-operation-request.ts";
import { USER_SCHEMA, USER_WIRE_NAMES } from "./fixtures.ts";
import { srcImport } from "./helpers.ts";

const SQLITE_HEADER_MAGIC = "SQLite format 3\u0000";

function removeTempDir(dir: string): void {
    try {
        rmSync(dir, { recursive: true, force: true, maxRetries: 10, retryDelay: 500 });
    } catch (error) {
        if (
            process.platform === "win32" &&
            error instanceof Error &&
            "code" in error &&
            (error.code === "EPERM" || error.code === "EBUSY")
        ) {
            return;
        }
        throw error;
    }
}

function ensureNativeOverride(): void {
    if (process.env.NAPI_RS_NATIVE_LIBRARY_PATH) {
        return;
    }
    const resolveScript = fileURLToPath(new URL("../../../../scripts/resolve-native-artifact.mjs", import.meta.url));
    const artifactOut = spawnSync(process.execPath, [resolveScript], {
        encoding: "utf8",
    });
    if (artifactOut.status === 0 && artifactOut.stdout.trim()) {
        process.env.NAPI_RS_NATIVE_LIBRARY_PATH = artifactOut.stdout.trim();
    }
}

async function loadNode() {
    ensureNativeOverride();
    return import(srcImport("src/node/index.ts"));
}

async function createExecutor(node: Awaited<ReturnType<typeof loadNode>>, dbPath: string) {
    return node.createIrisOperationExecutor({
        profile: "sqlite",
        sqlitePath: dbPath,
        schema: USER_SCHEMA,
        wireNamesByEntity: USER_WIRE_NAMES,
    });
}

test("openSqliteSession exposes the bundled sqlite engine identity", async () => {
    const node = await loadNode();
    const bindings = node.loadIrisNative();

    const session = bindings.openSqliteSession?.(":memory:");
    assert.ok(session);
    const engineVersion = session!.sqliteEngineVersion?.();
    const sourceId = session!.sqliteSourceId?.();
    assert.ok(engineVersion && engineVersion.length > 0);
    assert.ok(sourceId && sourceId.length > 0);
    session!.close();
});

test("generated sqlite flow covers managed push create findMany and reopen", async () => {
    const node = await loadNode();
    const dir = mkdtempSync(join(tmpdir(), "iris-sqlite-e2e-"));
    const dbPath = join(dir, "app.sqlite");

    const executor = await createExecutor(node, dbPath);

    const create = await executor.execute(
        buildOperationRequest(
            { operationId: "User.create", contractFingerprint: "unused" },
            declaredVosOperation(
                "User::insert({ user_id: $data_user_id, user_name: $data_user_name, active: $data_active })",
            ),
            { data_user_id: "e2e-1", data_user_name: "Ada", data_active: true },
        ),
    );
    assert.equal(create.ok, true);
    if (create.ok) {
        assert.equal(create.value[0]?.userId, "e2e-1");
        assert.equal(create.value[0]?.userName, "Ada");
    }

    const findMany = await executor.execute(
        buildOperationRequest(
            { operationId: "User.findMany", contractFingerprint: "unused" },
            declaredVosOperation("User.filter(x => x.active).collect()"),
        ),
    );
    assert.equal(findMany.ok, true);
    if (findMany.ok) {
        assert.equal(findMany.value.length, 1);
        assert.equal(findMany.value[0]?.userId, "e2e-1");
    }

    const updated = await executor.execute(
        buildOperationRequest(
            { operationId: "User.update", contractFingerprint: "unused" },
            declaredVosOperation(
                'User.filter(x => x.user_id == $p_user_id).patch({ user_name: $patch_user_name })',
            ),
            { p_user_id: "e2e-1", patch_user_name: "Grace" },
        ),
    );
    assert.equal(updated.ok, true);
    if (updated.ok) {
        assert.equal(updated.value[0]?.userName, "Grace");
    }

    await executor.close();

    const reopened = await createExecutor(node, dbPath);
    const again = await reopened.execute(
        buildOperationRequest(
            { operationId: "User.findMany", contractFingerprint: "unused" },
            declaredVosOperation('User.filter(x => x.user_id == $p_user_id).collect()'),
            { p_user_id: "e2e-1" },
        ),
    );
    assert.equal(again.ok, true);
    if (again.ok) {
        assert.equal(again.value.length, 1);
        assert.equal(again.value[0]?.userName, "Grace");
    }

    const deleted = await reopened.executeUnit(
        buildOperationRequest(
            { operationId: "User.delete", contractFingerprint: "unused" },
            declaredVosOperation('User.filter(x => x.user_id == $p_user_id).delete()'),
            { p_user_id: "e2e-1" },
        ),
    );
    assert.equal(deleted.ok, true);

    const afterDelete = await reopened.execute(
        buildOperationRequest(
            { operationId: "User.findMany", contractFingerprint: "unused" },
            declaredVosOperation('User.filter(x => x.user_id == $p_user_id).collect()'),
            { p_user_id: "e2e-1" },
        ),
    );
    assert.equal(afterDelete.ok, true);
    if (afterDelete.ok) {
        assert.equal(afterDelete.value.length, 0);
    }

    await reopened.close();
    removeTempDir(dir);
});

test("iris sqlite file carries the real SQLite format-3 header", async () => {
    const node = await loadNode();
    const dir = mkdtempSync(join(tmpdir(), "iris-sqlite-header-"));
    const dbPath = join(dir, "header.sqlite");

    const executor = await createExecutor(node, dbPath);
    const created = await executor.execute(
        buildOperationRequest(
            { operationId: "User.create", contractFingerprint: "unused" },
            declaredVosOperation(
                "User::insert({ user_id: $data_user_id, user_name: $data_user_name, active: $data_active })",
            ),
            { data_user_id: "header", data_user_name: "Format", data_active: true },
        ),
    );
    assert.equal(created.ok, true);
    await executor.close();

    const bytes = readFileSync(dbPath);
    assert.ok(bytes.length >= 512);
    assert.equal(bytes.subarray(0, 16).toString("latin1"), SQLITE_HEADER_MAGIC);
    const pageSize = bytes.readUInt16BE(16);
    assert.ok(pageSize === 65536 || (pageSize >= 512 && (pageSize & (pageSize - 1)) === 0));
    const pageCount = bytes.readUInt32BE(28);
    assert.ok(pageCount >= 1);
    assert.equal(bytes.length, pageSize * pageCount);

    const reopened = await createExecutor(node, dbPath);
    const rows = await reopened.execute(
        buildOperationRequest(
            { operationId: "User.findMany", contractFingerprint: "unused" },
            declaredVosOperation("User.filter(x => x.user_name == $name).collect()"),
            { name: "Format" },
        ),
    );
    assert.equal(rows.ok, true);
    if (rows.ok) {
        assert.equal(rows.value.length, 1);
    }
    await reopened.close();

    removeTempDir(dir);
});

test("iris sqlite file keeps committed rows across an independent reopen", async () => {
    const node = await loadNode();
    const dir = mkdtempSync(join(tmpdir(), "iris-sqlite-persist-"));
    const dbPath = join(dir, "persist.sqlite");

    const writer = await createExecutor(node, dbPath);
    const created = await writer.execute(
        buildOperationRequest(
            { operationId: "User.create", contractFingerprint: "unused" },
            declaredVosOperation(
                "User::insert({ user_id: $data_user_id, user_name: $data_user_name, active: $data_active })",
            ),
            { data_user_id: "persist-1", data_user_name: "Persisted", data_active: true },
        ),
    );
    assert.equal(created.ok, true);
    await writer.close();

    const reader = await createExecutor(node, dbPath);
    const rows = await reader.execute(
        buildOperationRequest(
            { operationId: "User.findMany", contractFingerprint: "unused" },
            declaredVosOperation('User.filter(x => x.user_id == $p_user_id).collect()'),
            { p_user_id: "persist-1" },
        ),
    );
    assert.equal(rows.ok, true);
    if (rows.ok) {
        assert.equal(rows.value.length, 1);
        assert.equal(rows.value[0]?.userName, "Persisted");
    }
    await reader.close();

    removeTempDir(dir);
});
