import assert from "node:assert/strict";
import { test } from "node:test";

import { buildOperationRequest, declaredVosOperation } from "../src/runtime/build-operation-request.ts";
import { stringifyOperationRequest } from "../src/runtime/operation-parameters.ts";
import { guardOperationExecutor, validateContractFingerprint } from "../src/runtime/validate-contract.ts";
import type { OperationExecutor } from "../src/types/operation-executor.ts";

const FINGERPRINT = "fp-test-abc123";

function sampleRequest(fingerprint = FINGERPRINT) {
    return buildOperationRequest(
        { operationId: "user.findMany", contractFingerprint: fingerprint },
        declaredVosOperation("User.filter(x => x.active).collect()"),
    );
}

test("validateContractFingerprint accepts matching fingerprint", () => {
    assert.equal(validateContractFingerprint(sampleRequest(), FINGERPRINT), null);
});

test("validateContractFingerprint rejects mismatch", () => {
    const diagnostic = validateContractFingerprint(sampleRequest("other-fp"), FINGERPRINT);
    assert.ok(diagnostic);
    assert.equal(diagnostic!.code, "IRIS-SCHEMA-MISMATCH");
    assert.equal(diagnostic!.diagnosticCode, "IRIS-CONTRACT-MISMATCH");
    assert.match(diagnostic!.message, /other-fp/);
});

test("guardOperationExecutor rejects mismatched requests before delegate", async () => {
    let delegateCalls = 0;
    const delegate: OperationExecutor = {
        async execute() {
            delegateCalls += 1;
            return { ok: true, value: [] };
        },
        async executeUnit() {
            delegateCalls += 1;
            return { ok: true, value: undefined };
        },
        async begin() {
            return { ok: true, value: undefined };
        },
        async commit() {
            return { ok: true, value: undefined };
        },
        async rollback() {
            return { ok: true, value: undefined };
        },
        async close() {},
    };

    const guarded = guardOperationExecutor(delegate, FINGERPRINT);
    const bad = await guarded.execute(sampleRequest("wrong"));
    assert.equal(bad.ok, false);
    assert.equal(delegateCalls, 0);
    if (!bad.ok) {
        assert.equal(bad.diagnostics[0]?.code, "IRIS-SCHEMA-MISMATCH");
        assert.equal(bad.diagnostics[0]?.diagnosticCode, "IRIS-CONTRACT-MISMATCH");
    }

    const good = await guarded.execute(sampleRequest());
    assert.equal(good.ok, true);
    assert.equal(delegateCalls, 1);

    const badUnit = await guarded.executeUnit(sampleRequest("wrong"));
    assert.equal(badUnit.ok, false);
    assert.equal(delegateCalls, 1);
});

// 生成客户端主 ABI 的键名属于协议范畴，这里逐字钉死 camelCase 拼写，
// 防止后续把核心类型的 Rust 命名透传到 wire 上。
test("operation request ABI keeps camelCase keys and kebab-case kind", () => {
    const request = buildOperationRequest(
        { operationId: "user.findMany", contractFingerprint: FINGERPRINT },
        declaredVosOperation("User.filter(x => x.active).collect()"),
        { limit: 3 },
        250,
    );

    assert.deepEqual(Object.keys(request), ["identity", "operation", "parameters", "deadlineMs"]);
    assert.deepEqual(Object.keys(request.identity), ["operationId", "contractFingerprint"]);
    assert.deepEqual(Object.keys(request.operation), ["kind", "source"]);
    assert.equal(request.operation.kind, "declared-vos");

    const wire = JSON.parse(stringifyOperationRequest(request)) as Record<string, unknown>;
    assert.deepEqual(Object.keys(wire), ["identity", "operation", "deadlineMs", "parameters"]);
    assert.deepEqual(wire.identity, { operationId: "user.findMany", contractFingerprint: FINGERPRINT });
    assert.deepEqual(wire.operation, { kind: "declared-vos", source: "User.filter(x => x.active).collect()" });
    assert.equal(wire.deadlineMs, 250);
});
