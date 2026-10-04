import assert from "node:assert/strict";
import { test } from "node:test";

import { createOperationExecutorFromSession } from "../src/runtime/operation-binding.ts";
import { buildOperationRequest, declaredVosOperation } from "../src/runtime/build-operation-request.ts";
import type { MemorySessionBinding, SessionExecuteWire } from "../src/bindings.ts";

function wire(ok: boolean, rowsJson = "[]", error?: string): SessionExecuteWire {
    return { ok, rowsJson, error: error ?? null };
}

test("executeUnit routes declared-vos through session execute instead of executeOperation", async () => {
    let operationCalls = 0;
    let executeCalls = 0;
    const session: MemorySessionBinding = {
        executeOperation() {
            operationCalls += 1;
            return wire(true, '[{"user_id":"via-operation"}]');
        },
        execute() {
            executeCalls += 1;
            return wire(true);
        },
        query() {
            return wire(true, "[]");
        },
        executeVos() {
            return wire(true, "[]");
        },
        close() {},
    };

    const executor = createOperationExecutorFromSession(session);
    const request = buildOperationRequest(
        { operationId: "$execute", contractFingerprint: "fp" },
        declaredVosOperation("User.filter(x => x.active).collect()"),
    );
    const result = await executor.executeUnit(request);

    assert.equal(result.ok, true);
    assert.equal(executeCalls, 1);
    assert.equal(operationCalls, 0);
});

test("createIrisOperationExecutor executeUnit runs declared-vos on sqlite", async () => {
    const node = await import(new URL("../src/node/index.ts", import.meta.url).href);
    const { USER_SCHEMA } = await import(new URL("./fixtures.ts", import.meta.url).href);
    const { buildOperationRequest, declaredVosOperation } = await import(
        new URL("../src/runtime/build-operation-request.ts", import.meta.url).href
    );

    let executor;
    try {
        executor = await node.createIrisOperationExecutor({
            profile: "sqlite",
            sqlitePath: ":memory:",
            schema: USER_SCHEMA,
        });
    } catch (error) {
        if (error instanceof Error && "code" in error && error.code === "native-package-missing") {
            return;
        }
        throw error;
    }

    const result = await executor.executeUnit(
        buildOperationRequest(
            { operationId: "$execute", contractFingerprint: "unused" },
            declaredVosOperation("User.filter(x => x.active).collect()"),
        ),
    );
    assert.equal(result.ok, true);
    await executor.close();
});

test("createIrisOperationExecutor runs generated delete on sqlite", async () => {
    const node = await import(new URL("../src/node/index.ts", import.meta.url).href);
    const { USER_SCHEMA } = await import(new URL("./fixtures.ts", import.meta.url).href);
    const { buildOperationRequest, declaredVosOperation } = await import(
        new URL("../src/runtime/build-operation-request.ts", import.meta.url).href
    );

    let executor;
    try {
        executor = await node.createIrisOperationExecutor({
            profile: "sqlite",
            sqlitePath: ":memory:",
            schema: USER_SCHEMA,
        });
    } catch (error) {
        if (error instanceof Error && "code" in error && error.code === "native-package-missing") {
            return;
        }
        throw error;
    }

    const create = await executor.execute(
        buildOperationRequest(
            { operationId: "User.create", contractFingerprint: "unused" },
            declaredVosOperation(
                'User::insert({ user_id: $data_user_id, user_name: $data_user_name, active: $data_active })',
            ),
            { data_user_id: "u-del", data_user_name: "Temp", data_active: true },
        ),
    );
    assert.equal(create.ok, true);

    const deleted = await executor.executeUnit(
        buildOperationRequest(
            { operationId: "User.delete", contractFingerprint: "unused" },
            declaredVosOperation('User.filter(x => x.user_id == $p_user_id).delete()'),
            { p_user_id: "u-del" },
        ),
    );
    assert.equal(deleted.ok, true);

    const read = await executor.execute(
        buildOperationRequest(
            { operationId: "User.findMany", contractFingerprint: "unused" },
            declaredVosOperation('User.filter(x => x.user_id == $p_user_id).collect()'),
            { p_user_id: "u-del" },
        ),
    );
    assert.equal(read.ok, true);
    if (read.ok) {
        assert.equal(read.value.length, 0);
    }
    await executor.close();
});
