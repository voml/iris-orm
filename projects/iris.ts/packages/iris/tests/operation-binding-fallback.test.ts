import assert from "node:assert/strict";
import { test } from "node:test";

import { createOperationExecutorFromSession } from "../src/runtime/operation-binding.ts";
import { buildOperationRequest, declaredVosOperation } from "../src/runtime/build-operation-request.ts";
import type { MemorySessionBinding, SessionExecuteWire } from "../src/bindings.ts";

const I64_MAX = 9223372036854775807n;
const OP_ID = "op_" + "c".repeat(64);

function wire(ok: boolean, rowsJson = "[]", error?: string): SessionExecuteWire {
    return { ok, rowsJson, error: error ?? null };
}

function queryOnlySession(rows: SessionExecuteWire = wire(true, "[]")): {
    session: MemorySessionBinding;
    queries: Array<{ source: string; parametersJson?: string | null }>;
} {
    const queries: Array<{ source: string; parametersJson?: string | null }> = [];
    return {
        queries,
        session: {
            query(source, parametersJson) {
                queries.push({ source, parametersJson });
                return rows;
            },
            executeVos(source, parametersJson) {
                queries.push({ source, parametersJson });
                return rows;
            },
            close() {},
        },
    };
}

test("session fallback without `executeOperation` refuses non declared-vos operations", async () => {
    const { session, queries } = queryOnlySession();
    const executor = createOperationExecutorFromSession(session);

    const result = await executor.execute(
        buildOperationRequest(
            { operationId: "User.findMany", contractFingerprint: "fp" },
            { kind: "find-many", entity: "User" },
        ),
    );
    assert.equal(result.ok, false);
    if (!result.ok) {
        assert.equal(result.diagnostics[0]?.code, "IRIS-INVALID-ARGUMENT");
        assert.match(result.diagnostics[0]?.message ?? "", /session fallback requires declared-vos operations/);
    }
    assert.equal(queries.length, 0, "the session must not be touched for refused operations");
});

test("session fallback runs declared-vos through `query` with tagged parameters", async () => {
    const rows = wire(true, '[{"embedding":{"$database":"vector","dimension":2,"components":"0000803f00000040"}}]');
    const { session, queries } = queryOnlySession(rows);
    const executor = createOperationExecutorFromSession(session, {
        wireNamesByEntity: { Search: { embedding: "embedding" } },
        valueCarriersByEntity: { Search: { embedding: "vector" } },
    });

    const result = await executor.execute(
        buildOperationRequest(
            { operationId: "Search.run", contractFingerprint: "fp" },
            declaredVosOperation("search($p_embedding)"),
            { p_embedding: new Float32Array([1, 2]) },
        ),
    );
    assert.equal(result.ok, true);
    if (!result.ok) {
        return;
    }
    assert.ok(result.value[0]?.embedding instanceof Float32Array);
    assert.deepEqual(Array.from(result.value[0]?.embedding as Float32Array), [1, 2]);

    assert.equal(queries.length, 1);
    assert.equal(queries[0]?.source, "search($p_embedding)");
    assert.equal(queries[0]?.parametersJson, '{"p_embedding":{"$database":"vector","dimension":2,"components":"0000803f00000040"}}');
});

test("string wire results and failures become envelopes", async () => {
    const failing: MemorySessionBinding = {
        executeOperation() {
            return '{"ok":false,"error":"boom"}';
        },
        executeVos() {
            return wire(true);
        },
        close() {},
    };
    const failedExecutor = createOperationExecutorFromSession(failing);
    const failed = await failedExecutor.execute(
        buildOperationRequest({ operationId: "User.findMany", contractFingerprint: "fp" }, declaredVosOperation("User.collect()")),
    );
    assert.equal(failed.ok, false);
    if (!failed.ok) {
        assert.equal(failed.diagnostics[0]?.code, "IRIS-INTERNAL");
        assert.equal(failed.diagnostics[0]?.message, "boom");
    }

    const succeeding: MemorySessionBinding = {
        executeOperation() {
            return '{"ok":true,"rows":[{"user_id":"u1"}]}';
        },
        executeVos() {
            return wire(true);
        },
        close() {},
    };
    const okExecutor = createOperationExecutorFromSession(succeeding, {
        wireNamesByEntity: { User: { userId: "user_id" } },
    });
    const succeeded = await okExecutor.execute(
        buildOperationRequest({ operationId: "User.findMany", contractFingerprint: "fp" }, declaredVosOperation("User.collect()")),
    );
    assert.equal(succeeded.ok, true);
    if (succeeded.ok) {
        assert.equal(succeeded.value[0]?.userId, "u1");
    }
});

test("parameter encoding failures are reported before the fallback session is touched", async () => {
    const { session, queries } = queryOnlySession();
    const executor = createOperationExecutorFromSession(session);

    const result = await executor.execute(
        buildOperationRequest(
            { operationId: "User.findMany", contractFingerprint: "fp" },
            declaredVosOperation("User.filter(x => x.seq == $p_seq).collect()"),
            { p_seq: I64_MAX + 1n },
        ),
    );
    assert.equal(result.ok, false);
    if (!result.ok) {
        assert.equal(result.diagnostics[0]?.code, "IRIS-INVALID-ARGUMENT");
        assert.equal(result.diagnostics[0]?.diagnosticCode, "IRIS-PARAM-ENCODING");
        assert.match(result.diagnostics[0]?.message ?? "", /outside the VOS `i64` range/);
    }
    assert.equal(queries.length, 0);
});

test("escape hatches are refused on the fallback path without querying", async () => {
    const { session, queries } = queryOnlySession();
    const executor = createOperationExecutorFromSession(session);

    const queryResult = await executor.execute(
        buildOperationRequest({ operationId: "$query", contractFingerprint: "fp" }, declaredVosOperation("anything")),
    );
    assert.equal(queryResult.ok, false);
    if (!queryResult.ok) {
        assert.equal(queryResult.diagnostics[0]?.code, "IRIS-INVALID-ARGUMENT");
        assert.equal(queryResult.diagnostics[0]?.diagnosticCode, "IRIS-ESCAPE-HATCH-REMOVED");
    }

    const unitResult = await executor.executeUnit(
        buildOperationRequest({ operationId: "$execute", contractFingerprint: "fp" }, declaredVosOperation("anything")),
    );
    assert.equal(unitResult.ok, false);
    if (!unitResult.ok) {
        assert.equal(unitResult.diagnostics[0]?.code, "IRIS-INVALID-ARGUMENT");
        assert.equal(unitResult.diagnostics[0]?.diagnosticCode, "IRIS-ESCAPE-HATCH-REMOVED");
    }
    assert.equal(queries.length, 0);
});

test("`executeUnit` falls back to the query surface when the session has no `execute`", async () => {
    const { session, queries } = queryOnlySession(wire(true, '[{"user_id":"u1"}]'));
    const executor = createOperationExecutorFromSession(session);

    const result = await executor.executeUnit(
        buildOperationRequest({ operationId: "User.delete", contractFingerprint: "fp" }, declaredVosOperation("User.delete()")),
    );
    assert.equal(result.ok, true);
    assert.equal(queries.length, 1);
    assert.equal(queries[0]?.source, "User.delete()");
});

test("wire failures forward the author code, semantic kind and diagnostic", async () => {
    const objectSession: MemorySessionBinding = {
        executeOperation() {
            return {
                ok: false,
                rowsJson: "[]",
                error: "duplicate user_name",
                errorCode: "IRIS-UNIQUE-CONFLICT",
                errorKind: "unique_conflict",
                errorDiagnosticCode: "23505",
            };
        },
        executeVos() {
            return wire(true);
        },
        close() {},
    };
    const objectResult = await createOperationExecutorFromSession(objectSession).execute(
        buildOperationRequest({ operationId: "User.create", contractFingerprint: "fp" }, declaredVosOperation("User::insert({})")),
    );
    assert.equal(objectResult.ok, false);
    if (!objectResult.ok) {
        assert.equal(objectResult.diagnostics[0]?.code, "IRIS-UNIQUE-CONFLICT");
        assert.equal(objectResult.diagnostics[0]?.kind, "unique_conflict");
        assert.equal(objectResult.diagnostics[0]?.diagnosticCode, "23505");
    }

    const stringSession: MemorySessionBinding = {
        executeOperation() {
            return '{"ok":false,"rows":[],"error":"duplicate user_name","errorCode":"IRIS-UNIQUE-CONFLICT","errorKind":"unique_conflict","errorDiagnosticCode":"23505"}';
        },
        executeVos() {
            return wire(true);
        },
        close() {},
    };
    const stringResult = await createOperationExecutorFromSession(stringSession).execute(
        buildOperationRequest({ operationId: "User.create", contractFingerprint: "fp" }, declaredVosOperation("User::insert({})")),
    );
    assert.equal(stringResult.ok, false);
    if (!stringResult.ok) {
        assert.equal(stringResult.diagnostics[0]?.code, "IRIS-UNIQUE-CONFLICT");
        assert.equal(stringResult.diagnostics[0]?.kind, "unique_conflict");
        assert.equal(stringResult.diagnostics[0]?.diagnosticCode, "23505");
    }
});

test("transaction control reports `IRIS-TRANSACTION-FAILED` with the host diagnostic", async () => {
    const unsupported: MemorySessionBinding = {
        executeVos() {
            return wire(true);
        },
        close() {},
    };
    const missing = await createOperationExecutorFromSession(unsupported).begin();
    assert.equal(missing.ok, false);
    if (!missing.ok) {
        assert.equal(missing.diagnostics[0]?.code, "IRIS-TRANSACTION-FAILED");
        assert.equal(missing.diagnostics[0]?.kind, "transaction_failed");
        assert.equal(missing.diagnostics[0]?.diagnosticCode, "IRIS-TRANSACTION-UNSUPPORTED");
    }

    const alreadyOpen: MemorySessionBinding = {
        executeVos() {
            return wire(true);
        },
        begin() {
            return {
                ok: false,
                rowsJson: "[]",
                error: "a transaction is already open",
                errorCode: "IRIS-TRANSACTION-FAILED",
                errorKind: "transaction_failed",
                errorDiagnosticCode: "IRIS-TRANSACTION-ALREADY-OPEN",
            };
        },
        close() {},
    };
    const forward = await createOperationExecutorFromSession(alreadyOpen).begin();
    assert.equal(forward.ok, false);
    if (!forward.ok) {
        assert.equal(forward.diagnostics[0]?.code, "IRIS-TRANSACTION-FAILED");
        assert.equal(forward.diagnostics[0]?.diagnosticCode, "IRIS-TRANSACTION-ALREADY-OPEN");
    }
});

test("durable `op_<hex>` identities keep wire rows outside the entity name map", async () => {
    const calls: string[] = [];
    const session: MemorySessionBinding = {
        executeOperation(operationJson) {
            calls.push(operationJson);
            return wire(true, '[{"user_id":"u1"}]');
        },
        executeVos() {
            return wire(true);
        },
        close() {},
    };
    const executor = createOperationExecutorFromSession(session, {
        wireNamesByEntity: { User: { userId: "user_id" } },
        valueCarriersByEntity: { User: { userId: "uuid" } },
    });

    const result = await executor.execute(
        buildOperationRequest({ operationId: OP_ID, contractFingerprint: "fp" }, declaredVosOperation("search($p_q)"), { p_q: "x" }),
    );
    assert.equal(result.ok, true);
    if (!result.ok) {
        return;
    }
    // No `Entity.operation` prefix means no generated wire table applies.
    assert.equal(result.value[0]?.user_id, "u1");
    assert.equal(result.value[0]?.userId, undefined);
    assert.match(calls[0]!, new RegExp(`"operationId":"${OP_ID}"`));
});
