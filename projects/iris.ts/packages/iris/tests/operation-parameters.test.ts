import assert from "node:assert/strict";
import { test } from "node:test";

import { createOperationExecutorFromSession } from "../src/runtime/operation-binding.ts";
import {
    IrisParameterEncodingError,
    exactI64Text,
    stringifyOperationParameters,
    stringifyOperationRequest,
} from "../src/runtime/operation-parameters.ts";
import { bindD1Parameters, coerceD1BindValue } from "../src/cloudflare/d1-plan.ts";
import { buildOperationRequest, declaredVosOperation } from "../src/runtime/build-operation-request.ts";
import type { MemorySessionBinding, SessionExecuteWire } from "../src/bindings.ts";
import type { OperationRequest } from "../src/types/contract.ts";

const I64_MAX = 9223372036854775807n;

function request(parameters?: Readonly<Record<string, unknown>>): OperationRequest {
    return buildOperationRequest(
        { operationId: "Metric.findMany", contractFingerprint: "fp" },
        declaredVosOperation("Metric.filter(x => x.seq == $p_seq).collect()"),
        parameters,
    );
}

function recordingSession(rowsJson: string): { session: MemorySessionBinding; requests: string[] } {
    const requests: string[] = [];
    return {
        requests,
        session: {
            executeOperation(json: string): SessionExecuteWire {
                requests.push(json);
                return { ok: true, rowsJson, error: null };
            },
            executeVos(): SessionExecuteWire {
                return { ok: true, rowsJson: "[]", error: null };
            },
            close() {},
        },
    };
}

test("parameter encoding keeps i64 as exact decimal digits", () => {
    const encoded = stringifyOperationParameters({ p_seq: I64_MAX });
    assert.equal(encoded, '{"p_seq":9223372036854775807}');
});

test("parameter encoding carries bytes as base64 instead of a stringified view", () => {
    const encoded = stringifyOperationParameters({ p_payload: new Uint8Array([1, 2, 3]) });
    assert.equal(encoded, '{"p_payload":"AQID"}');
    assert.equal(JSON.stringify({ p_payload: new Uint8Array([1, 2, 3]) }), '{"p_payload":{"0":1,"1":2,"2":3}}');
});

test("parameter encoding keeps scalars and drops undefined entries", () => {
    const encoded = stringifyOperationParameters({ a: null, b: true, c: 1.5, d: "text", e: undefined });
    assert.equal(encoded, '{"a":null,"b":true,"c":1.5,"d":"text"}');
    assert.equal(stringifyOperationParameters(), "{}");
});

test("parameter encoding rejects values the binder cannot express", () => {
    assert.throws(() => stringifyOperationParameters({ p: { nested: 1 } }), IrisParameterEncodingError);
    assert.throws(() => stringifyOperationParameters({ p: [1] }), IrisParameterEncodingError);
    assert.throws(() => stringifyOperationParameters({ p: Number.NaN }), IrisParameterEncodingError);
    assert.throws(
        () => stringifyOperationParameters({ p: I64_MAX + 1n }),
        (error: unknown) =>
            error instanceof IrisParameterEncodingError && error.message.includes("outside the VOS `i64` range"),
    );
    assert.equal(exactI64Text(I64_MAX, "parameter `p`"), "9223372036854775807");
});

test("request encoding splices exact parameters into the session payload", () => {
    const encoded = stringifyOperationRequest(request({ p_seq: I64_MAX, p_payload: new Uint8Array([255]) }));
    const parsed = JSON.parse(encoded.replace("9223372036854775807", '"9223372036854775807"')) as {
        operation: { kind: string };
        parameters: Record<string, unknown>;
    };
    assert.equal(parsed.operation.kind, "declared-vos");
    assert.equal(parsed.parameters.p_seq, "9223372036854775807");
    assert.equal(parsed.parameters.p_payload, "/w==");
    assert.match(encoded, /"p_seq":9223372036854775807/);
});

test("session executor binds bigint and bytes parameters without demoting them", async () => {
    const rows = '[{"metric_id":"m1","seq":"9223372036854775807","payload":"AQID"}]';
    const { session, requests } = recordingSession(rows);
    const executor = createOperationExecutorFromSession(session, {
        wireNamesByEntity: { Metric: { metricId: "metric_id", seq: "seq", payload: "payload" } },
        valueCarriersByEntity: { Metric: { seq: "i64", payload: "bytes" } },
    });

    const result = await executor.execute(request({ p_seq: I64_MAX, p_payload: new Uint8Array([1, 2, 3]) }));
    assert.equal(result.ok, true);
    if (!result.ok) {
        return;
    }
    const row = result.value[0]!;
    assert.equal(typeof row.seq, "bigint");
    assert.equal(row.seq, I64_MAX);
    assert.ok(row.payload instanceof Uint8Array);
    assert.deepEqual(Array.from(row.payload as Uint8Array), [1, 2, 3]);

    const sent = requests[0]!;
    assert.match(sent, /"p_seq":9223372036854775807/);
    assert.match(sent, /"p_payload":"AQID"/);
});

test("session executor reports an out-of-range bigint as a structured diagnostic", async () => {
    const { session } = recordingSession("[]");
    const executor = createOperationExecutorFromSession(session);
    const result = await executor.execute(request({ p_seq: I64_MAX + 1n }));
    assert.equal(result.ok, false);
    if (result.ok) {
        return;
    }
    assert.equal(result.diagnostics[0]?.code, "IRIS-PARAM-ENCODING");
    assert.match(result.diagnostics[0]?.message ?? "", /outside the VOS `i64` range/);
});

test("D1 binding sends i64 as exact text and bytes as ArrayBuffer", () => {
    const payload = new Uint8Array([1, 2, 3]);
    const bound = bindD1Parameters(
        {
            sql: "SELECT seq, payload FROM Metric WHERE seq = ?",
            mode: "read",
            paramOrder: ["p_seq", "p_payload", "p_active"],
        },
        { p_seq: I64_MAX, p_payload: payload, p_active: true },
    );
    assert.equal(bound[0], "9223372036854775807");
    assert.ok(bound[1] instanceof ArrayBuffer);
    assert.deepEqual(Array.from(new Uint8Array(bound[1] as ArrayBuffer)), [1, 2, 3]);
    assert.equal(bound[2], 1);
    assert.equal(coerceD1BindValue("plain"), "plain");
    assert.throws(() => coerceD1BindValue(I64_MAX + 1n, "p_seq"), IrisParameterEncodingError);
});
