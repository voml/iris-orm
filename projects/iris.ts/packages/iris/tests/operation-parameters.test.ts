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
import { decodeWireValueByCarrier } from "../src/runtime/value-carrier.ts";
import type { MemorySessionBinding, SessionExecuteWire } from "../src/bindings.ts";
import type { OperationRequest } from "../src/types/contract.ts";

const I64_MAX = 9223372036854775807n;

/** Payloads a text carrier would corrupt, plus the YYDB inline/object-store boundary. */
const BYTE_CASES: ReadonlyArray<readonly [string, Uint8Array]> = [
    ["empty", new Uint8Array(0)],
    ["nul", new Uint8Array([0x00, 0x00, 0x00])],
    ["invalid-utf8-with-nul", new Uint8Array([0x00, 0xff, 0x41, 0xc3, 0x28])],
    ["all-256", new Uint8Array(Array.from({ length: 256 }, (_, index) => index))],
    ["4097", new Uint8Array(Array.from({ length: 4097 }, (_, index) => index & 0xff))],
];

function lowercaseHex(bytes: Uint8Array): string {
    return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

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

test("parameter encoding carries bytes as the canonical tagged hex value", () => {
    const encoded = stringifyOperationParameters({ p_payload: new Uint8Array([1, 2, 3]) });
    assert.equal(encoded, '{"p_payload":{"$yydb":"bytes","hex":"010203"}}');
    assert.equal(JSON.stringify({ p_payload: new Uint8Array([1, 2, 3]) }), '{"p_payload":{"0":1,"1":2,"2":3}}');
});

test("every `bytes` parameter round trips through the tagged hex carrier", () => {
    for (const [label, bytes] of BYTE_CASES) {
        const encoded = stringifyOperationParameters({ p_payload: bytes });
        assert.equal(
            encoded,
            `{"p_payload":{"$yydb":"bytes","hex":${JSON.stringify(lowercaseHex(bytes))}}}`,
            label,
        );
        const parsed = JSON.parse(encoded) as { p_payload: unknown };
        const decoded = decodeWireValueByCarrier("bytes", parsed.p_payload, "payload");
        assert.ok(decoded instanceof Uint8Array, label);
        assert.equal(decoded.length, bytes.length, label);
        assert.deepEqual(Array.from(decoded), Array.from(bytes), label);
    }
});

test("`bytes` decoding rejects every non-canonical shape", () => {
    assert.throws(
        () => decodeWireValueByCarrier("bytes", { $yydb: "bytes", hex: "ABCD" }),
        /canonical lowercase hex/,
    );
    assert.throws(() => decodeWireValueByCarrier("bytes", { $yydb: "bytes", hex: "0" }), /canonical lowercase hex/);
    assert.throws(() => decodeWireValueByCarrier("bytes", { $yydb: "bytes" }), /without a `hex` string/);
    assert.throws(() => decodeWireValueByCarrier("bytes", { hex: "00" }), /not the tagged `bytes` carrier/);
    assert.throws(() => decodeWireValueByCarrier("bytes", "AQID"), /cannot convert string value to Uint8Array/);
    assert.equal(decodeWireValueByCarrier("bytes", null), null);
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
    assert.deepEqual(parsed.parameters.p_payload, { $yydb: "bytes", hex: "ff" });
    assert.match(encoded, /"p_seq":9223372036854775807/);
});

test("session executor binds bigint and bytes parameters without demoting them", async () => {
    const rows = '[{"metric_id":"m1","seq":"9223372036854775807","payload":{"$yydb":"bytes","hex":"010203"}}]';
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
    assert.match(sent, /"p_payload":\{"\$yydb":"bytes","hex":"010203"\}/);
});

test("session executor keeps every `bytes` result byte for byte", async () => {
    for (const [label, bytes] of BYTE_CASES) {
        const rows = JSON.stringify([
            { metric_id: "m1", seq: "1", payload: { $yydb: "bytes", hex: lowercaseHex(bytes) } },
        ]);
        const { session } = recordingSession(rows);
        const executor = createOperationExecutorFromSession(session, {
            wireNamesByEntity: { Metric: { metricId: "metric_id", seq: "seq", payload: "payload" } },
            valueCarriersByEntity: { Metric: { seq: "i64", payload: "bytes" } },
        });

        const result = await executor.execute(request({ p_seq: 1n, p_payload: bytes }));
        assert.equal(result.ok, true, label);
        if (!result.ok) {
            return;
        }
        const payload = result.value[0]!.payload;
        assert.ok(payload instanceof Uint8Array, label);
        assert.deepEqual(Array.from(payload as Uint8Array), Array.from(bytes), label);
    }
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
