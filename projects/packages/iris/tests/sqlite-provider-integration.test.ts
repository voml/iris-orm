import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { test } from "node:test";

import { buildOperationRequest, declaredVosOperation } from "../src/runtime/build-operation-request.ts";
import { USER_SCHEMA, USER_WIRE_NAMES } from "./fixtures.ts";
import { srcImport } from "./helpers.ts";

const CONTRACT_VERSION = "1.0.0";
const IRIS_ORM_ROOT = fileURLToPath(new URL("../../../..", import.meta.url));
const YYDS_SQLITE_NODE = join(IRIS_ORM_ROOT, "..", "yyds", "projects", "packages", "sqlite", "src", "node.ts");
const YYDS_SQLITE_CLI = join(IRIS_ORM_ROOT, "..", "yyds", "projects", "packages", "sqlite", "bin", "sqlite3.mjs");

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

function isNativePackageMissing(error: unknown): boolean {
    return error instanceof Error && "code" in error && error.code === "native-package-missing";
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

async function tryLoadYydsSqlite() {
    try {
        return await import(pathToFileURL(YYDS_SQLITE_NODE).href);
    } catch {
        return null;
    }
}

test("openSqliteSession exposes shared sqlite provider contract identity", async () => {
    const node = await loadNode();
    let bindings;
    try {
        bindings = node.loadIrisNative();
    } catch (error) {
        if (isNativePackageMissing(error)) {
            return;
        }
        throw error;
    }

    assert.equal(bindings.sqliteProviderContractVersion?.(), CONTRACT_VERSION);
    const session = bindings.openSqliteSession?.(":memory:");
    assert.ok(session);
    assert.equal(session!.sqliteProviderContractVersion?.(), CONTRACT_VERSION);
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

    let executor;
    try {
        executor = await node.createIrisOperationExecutor({
            profile: "sqlite",
            sqlitePath: dbPath,
            schema: USER_SCHEMA,
            wireNamesByEntity: USER_WIRE_NAMES,
            contractFingerprint: "unused",
        });
    } catch (error) {
        removeTempDir(dir);
        if (isNativePackageMissing(error)) {
            return;
        }
        throw error;
    }

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

    const reopened = await node.createIrisOperationExecutor({
        profile: "sqlite",
        sqlitePath: dbPath,
        schema: USER_SCHEMA,
        wireNamesByEntity: USER_WIRE_NAMES,
    });
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

test("iris sqlite file matches @yyds/sqlite engine version when native is available", async () => {
    const yyds = await tryLoadYydsSqlite();
    if (!yyds) {
        return;
    }

    const node = await loadNode();
    const dir = mkdtempSync(join(tmpdir(), "iris-sqlite-cross-"));
    const dbPath = join(dir, "cross.sqlite");

    let executor;
    try {
        executor = await node.createIrisOperationExecutor({
            profile: "sqlite",
            sqlitePath: dbPath,
            schema: USER_SCHEMA,
            wireNamesByEntity: USER_WIRE_NAMES,
        });
    } catch (error) {
        removeTempDir(dir);
        if (isNativePackageMissing(error)) {
            return;
        }
        throw error;
    }

    const bindings = node.loadIrisNative();
    const irisSession = bindings.openSqliteSession?.(dbPath);
    assert.ok(irisSession);
    const irisVersion = irisSession!.sqliteEngineVersion?.();
    irisSession!.close();

    await executor.execute(
        buildOperationRequest(
            { operationId: "User.create", contractFingerprint: "unused" },
            declaredVosOperation(
                "User::insert({ user_id: $data_user_id, user_name: $data_user_name, active: $data_active })",
            ),
            { data_user_id: "cross", data_user_name: "YYDS", data_active: true },
        ),
    );
    await executor.close();

    {
        const yydsDb = new yyds.SqliteConnection(dbPath);
        const yydsVersion = yydsDb.execute("SELECT sqlite_version()").rows[0][0];
        assert.equal(yydsVersion.kind, "text");
        if (yydsVersion.kind === "text") {
            assert.equal(String(yydsVersion.value), irisVersion);
        }

        const row = yydsDb.executeWithParameters('SELECT user_name FROM "User" WHERE user_id = ?1', [
            { kind: "text", value: "cross" },
        ]);
        assert.equal(row.rows[0][0].kind, "text");
        if (row.rows[0][0].kind === "text") {
            assert.equal(String(row.rows[0][0].value), "YYDS");
        }
    }

    removeTempDir(dir);
});

test("iris sqlite file is readable by @yyds/sqlite CLI", async () => {
    const yyds = await tryLoadYydsSqlite();
    if (!yyds) {
        return;
    }

    const node = await loadNode();
    const dir = mkdtempSync(join(tmpdir(), "iris-sqlite-cli-"));
    const dbPath = join(dir, "cli.sqlite");

    let executor;
    try {
        executor = await node.createIrisOperationExecutor({
            profile: "sqlite",
            sqlitePath: dbPath,
            schema: USER_SCHEMA,
            wireNamesByEntity: USER_WIRE_NAMES,
        });
    } catch (error) {
        removeTempDir(dir);
        if (isNativePackageMissing(error)) {
            return;
        }
        throw error;
    }

    await executor.execute(
        buildOperationRequest(
            { operationId: "User.create", contractFingerprint: "unused" },
            declaredVosOperation(
                "User::insert({ user_id: $data_user_id, user_name: $data_user_name, active: $data_active })",
            ),
            { data_user_id: "cli", data_user_name: "CLI", data_active: true },
        ),
    );
    await executor.close();

    const result = spawnSync(
        process.execPath,
        ["--experimental-strip-types", YYDS_SQLITE_CLI, dbPath, 'SELECT user_name FROM "User" WHERE user_id = \'cli\''],
        { encoding: "utf8" },
    );
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /CLI/);

    removeTempDir(dir);
});
