import assert from "node:assert/strict";
import { test } from "node:test";

import { createD1ReadOperationExecutor, createIrisOperationExecutor } from "../src/cloudflare/index.ts";
import { buildOperationRequest, declaredVosOperation } from "../src/runtime/build-operation-request.ts";
import { negotiateCapabilities } from "../src/runtime/negotiate-capabilities.ts";

const fakeD1 = {
    prepare() {
        return {
            bind() {
                return {
                    async all() {
                        return { results: [] };
                    },
                    async run() {
                        return { success: true };
                    },
                };
            },
        };
    },
};

test("negotiateCapabilities maps cloudflare-worker host to d1 profile", () => {
    const matrix = negotiateCapabilities({ host: "cloudflare-worker", profile: "d1", bindingReady: false });
    assert.equal(matrix.host, "cloudflare-worker");
    assert.equal(matrix.profile, "d1");
    assert.equal(matrix.storage.d1, true);
});

test("createIrisOperationExecutor requires d1 binding", async () => {
    await assert.rejects(
        () => createIrisOperationExecutor({ d1: undefined as never }),
        /d1 binding is required/,
    );
});

test("D1 read executor returns structured not-wired diagnostics", async () => {
    const executor = createD1ReadOperationExecutor(fakeD1);
    const request = buildOperationRequest(
        { operationId: "user.findMany", contractFingerprint: "fp" },
        declaredVosOperation("User.collect()"),
    );

    const read = await executor.execute(request);
    assert.equal(read.ok, false);
    if (!read.ok) {
        assert.equal(read.diagnostics[0]?.code, "IRIS-D1-READ-NOT-WIRED");
    }

    const write = await executor.executeUnit(request);
    assert.equal(write.ok, false);
    if (!write.ok) {
        assert.equal(write.diagnostics[0]?.code, "IRIS-D1-WRITE-NOT-WIRED");
    }
});

test("createIrisOperationExecutor applies contract fingerprint guard", async () => {
    const executor = await createIrisOperationExecutor({
        d1: fakeD1,
        contractFingerprint: "expected-fp",
    });
    const mismatch = await executor.execute(
        buildOperationRequest(
            { operationId: "user.findMany", contractFingerprint: "wrong-fp" },
            declaredVosOperation("User.collect()"),
        ),
    );
    assert.equal(mismatch.ok, false);
    if (!mismatch.ok) {
        assert.equal(mismatch.diagnostics[0]?.code, "IRIS-CONTRACT-MISMATCH");
    }
});
