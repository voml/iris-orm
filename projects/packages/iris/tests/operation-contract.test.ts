import assert from "node:assert/strict";
import { test } from "node:test";

import { buildOperationRequest, declaredVosOperation } from "../src/runtime/build-operation-request.ts";
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
    assert.equal(diagnostic!.code, "IRIS-CONTRACT-MISMATCH");
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
        async close() {},
    };

    const guarded = guardOperationExecutor(delegate, FINGERPRINT);
    const bad = await guarded.execute(sampleRequest("wrong"));
    assert.equal(bad.ok, false);
    assert.equal(delegateCalls, 0);
    if (!bad.ok) {
        assert.equal(bad.diagnostics[0]?.code, "IRIS-CONTRACT-MISMATCH");
    }

    const good = await guarded.execute(sampleRequest());
    assert.equal(good.ok, true);
    assert.equal(delegateCalls, 1);

    const badUnit = await guarded.executeUnit(sampleRequest("wrong"));
    assert.equal(badUnit.ok, false);
    assert.equal(delegateCalls, 1);
});
