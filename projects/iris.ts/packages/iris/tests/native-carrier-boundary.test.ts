import assert from "node:assert/strict";
import { test } from "node:test";

import { decodeWireValueByCarrier } from "../src/runtime/value-carrier.ts";
import {
    IrisParameterEncodingError,
    exactI64Text,
    stringifyOperationParameters,
    stringifyOperationRequest,
} from "../src/runtime/operation-parameters.ts";
import { buildOperationRequest, declaredVosOperation } from "../src/runtime/build-operation-request.ts";

const I64_MIN = -(2n ** 63n);
const I64_MAX = 2n ** 63n - 1n;
const UNSAFE_NUMBER = Number.MAX_SAFE_INTEGER + 2;

const DIGEST = "a".repeat(63) + "3";
const MANIFEST_HASH = "b".repeat(63) + "4";

function fileDescriptor(overrides: Record<string, unknown> = {}) {
    return {
        digest: DIGEST,
        octets: 10,
        manifest: { hash: MANIFEST_HASH, size: 128 },
        ...overrides,
    };
}

// ── decode side ────────────────────────────────────────────────────────────

test("`vector` decoding passes a `Float32Array` through as a defensive copy", () => {
    const source = new Float32Array([1.5, -2, 0.25]);
    const decoded = decodeWireValueByCarrier("vector", source, "embedding");
    assert.ok(decoded instanceof Float32Array);
    assert.notEqual(decoded, source, "the wire value must not be handed out by reference");
    assert.deepEqual(Array.from(decoded as Float32Array), [1.5, -2, 0.25]);
});

test("`vector` decoding refuses a missing `components` and a fractional `dimension`", () => {
    assert.throws(
        () => decodeWireValueByCarrier("vector", { $yydb: "vector", dimension: 1 }, "embedding"),
        /without a `components` hex string/,
    );
    assert.throws(
        () => decodeWireValueByCarrier("vector", { $yydb: "vector", dimension: 2.5, components: "0000803f" }, "embedding"),
        /non-positive `dimension`: 2.5/,
    );
    assert.throws(
        () => decodeWireValueByCarrier("vector", { $yydb: "vector", dimension: "2", components: "0000803f" }, "embedding"),
        /non-positive `dimension`/,
    );
});

test("`file` decoding refuses a missing `octets` and non-string identity members", () => {
    assert.throws(
        () => decodeWireValueByCarrier("file", { $yydb: "file", digest: DIGEST, manifest: { hash: MANIFEST_HASH, size: 1 } }, "photo"),
        /without an `octets` number/,
    );
    assert.throws(
        () => decodeWireValueByCarrier("file", { $yydb: "file", digest: 123, octets: 1, manifest: { hash: MANIFEST_HASH, size: 1 } }, "photo"),
        /without a `digest` string/,
    );
    assert.throws(
        () => decodeWireValueByCarrier("file", { $yydb: "file", digest: DIGEST, octets: 1, manifest: { hash: 7, size: 1 } }, "photo"),
        /whose `manifest.hash` is not a string/,
    );
    assert.throws(
        () => decodeWireValueByCarrier("file", { $yydb: "file", digest: DIGEST, octets: 1, mediaType: " ", manifest: { hash: MANIFEST_HASH, size: 1 } }, "photo"),
        /`mediaType` is not a non-blank string/,
    );
});

test("`file` decoding accepts exact decimal text and rejects beyond-2^53 numbers", () => {
    const decoded = decodeWireValueByCarrier(
        "file",
        { $yydb: "file", digest: DIGEST, octets: "9223372036854775807", manifest: { hash: MANIFEST_HASH, size: "123456789012345678" } },
        "photo",
    ) as { octets: bigint; manifest: { size: bigint } };
    assert.equal(decoded.octets, 9223372036854775807n);
    assert.equal(decoded.manifest.size, 123456789012345678n);

    assert.throws(
        () => decodeWireValueByCarrier("file", { $yydb: "file", digest: DIGEST, octets: UNSAFE_NUMBER, manifest: { hash: MANIFEST_HASH, size: 1 } }, "photo"),
        /beyond the 2\^53 exact range/,
    );
});

test("an unknown carrier label hard-fails instead of degrading to `unknown`", () => {
    assert.throws(
        () => decodeWireValueByCarrier("blob", "x", "thing"),
        (error: unknown) => {
            assert.ok(error instanceof Error);
            assert.match(error.message, /IRIS-VALUE-CARRIER: field `thing` carries unknown VOS label `blob`/);
            assert.match(error.message, /Known labels: bool,/);
            return true;
        },
    );
});

test("plain text is never reinterpreted as a native carrier", () => {
    const lookalike = '{"$yydb":"vector","dimension":2,"components":"0000803f00000040"}';
    assert.equal(decodeWireValueByCarrier("utf8", lookalike, "note"), lookalike);
    assert.equal(decodeWireValueByCarrier(undefined, lookalike, "note"), lookalike);
    assert.equal(decodeWireValueByCarrier("vector", null, "embedding"), null);
    assert.equal(decodeWireValueByCarrier("file", undefined, "photo"), null);
});

// ── encode side ────────────────────────────────────────────────────────────

test("a `Float32Array` with a non-finite component is refused at encoding", () => {
    assert.throws(
        () => stringifyOperationParameters({ p_embedding: new Float32Array([1, Number.NaN]) }),
        (error: unknown) =>
            error instanceof IrisParameterEncodingError && /parameter `p_embedding` is a `Float32Array` with a non-finite component/.test(error.message),
    );
    assert.throws(
        () => stringifyOperationParameters({ p_embedding: new Float32Array([Number.POSITIVE_INFINITY]) }),
        /non-finite component/,
    );
});

test("file encoding refuses negative, blank, inexact and missing members", () => {
    assert.throws(
        () => stringifyOperationParameters({ p_photo: fileDescriptor({ octets: -1n }) }),
        /`octets` is negative and outside the VOS `u64` range/,
    );
    assert.throws(
        () => stringifyOperationParameters({ p_photo: fileDescriptor({ octets: UNSAFE_NUMBER }) }),
        /outside the exact non-negative integer range/,
    );
    assert.throws(
        () => stringifyOperationParameters({ p_photo: fileDescriptor({ mediaType: "" }) }),
        /`mediaType` must be a non-blank string when present/,
    );
    assert.throws(
        () => stringifyOperationParameters({ p_photo: { digest: DIGEST, octets: 1, manifest: { hash: MANIFEST_HASH } } }),
        /`manifest.size` must be a number or bigint, not undefined/,
    );
});

test("an object that only looks like a descriptor is not encoded as a file", () => {
    assert.throws(
        () => stringifyOperationParameters({ p_photo: fileDescriptor({ octets: "10" }) }),
        IrisParameterEncodingError,
    );
    assert.throws(
        () => stringifyOperationParameters({ p_photo: fileDescriptor({ digest: 123 }) }),
        IrisParameterEncodingError,
    );
});

test("tagged-looking parameter text stays text through encoding", () => {
    const lookalike = `{"$yydb":"file","digest":"${DIGEST}","octets":10}`;
    const encoded = stringifyOperationParameters({ p_note: lookalike });
    assert.deepEqual(JSON.parse(encoded), { p_note: lookalike });
    assert.match(encoded, /^\{"p_note":"\{\\"\$yydb\\"/);
});

test("`exactI64Text` guards the `i64` endpoints", () => {
    assert.equal(exactI64Text(I64_MIN, "parameter `p`"), "-9223372036854775808");
    assert.equal(exactI64Text(I64_MAX, "parameter `p`"), "9223372036854775807");
    assert.throws(() => exactI64Text(I64_MIN - 1n, "parameter `p`"), IrisParameterEncodingError);
    assert.throws(() => exactI64Text(I64_MAX + 1n, "parameter `p`"), IrisParameterEncodingError);
});

// ── encode → decode round trips ────────────────────────────────────────────

test("a three-component `vector` round trips exactly through the tagged hex carrier", () => {
    const encoded = stringifyOperationParameters({ p_embedding: new Float32Array([1.5, -2, 0.25]) });
    const parsed = JSON.parse(encoded) as { p_embedding: unknown };
    assert.deepEqual(parsed.p_embedding, { $yydb: "vector", dimension: 3, components: "0000c03f000000c00000803e" });
    const decoded = decodeWireValueByCarrier("vector", parsed.p_embedding, "embedding");
    assert.ok(decoded instanceof Float32Array);
    assert.deepEqual(Array.from(decoded as Float32Array), [1.5, -2, 0.25]);
});

test("a file descriptor with big integers round trips with every bit intact", () => {
    const encoded = stringifyOperationParameters({
        p_photo: { digest: DIGEST, octets: 9223372036854775807n, name: "note.txt", manifest: { hash: MANIFEST_HASH, size: 4096n } },
    });
    assert.match(encoded, /"octets":9223372036854775807/);
    const parsed = JSON.parse(encoded.replace("9223372036854775807", '"9223372036854775807"')) as { p_photo: unknown };
    const decoded = decodeWireValueByCarrier("file", parsed.p_photo, "photo") as {
        digest: string;
        octets: bigint;
        name?: string;
        mediaType?: string;
        manifest: { hash: string; size: bigint };
    };
    assert.equal(decoded.digest, DIGEST);
    assert.equal(decoded.octets, 9223372036854775807n);
    assert.equal(decoded.name, "note.txt");
    assert.equal(decoded.mediaType, undefined);
    assert.equal(decoded.manifest.hash, MANIFEST_HASH);
    assert.equal(decoded.manifest.size, 4096n);
});

test("request encoding splices tagged vector text into the session payload", () => {
    const encoded = stringifyOperationRequest(
        buildOperationRequest(
            { operationId: "op_" + "f".repeat(64), contractFingerprint: "fp" },
            declaredVosOperation("search($p_embedding)"),
            { p_embedding: new Float32Array([1, 2]) },
        ),
    );
    assert.match(encoded, /"p_embedding":\{"\$yydb":"vector","dimension":2,"components":"0000803f00000040"\}/);
    assert.match(encoded, /"kind":"declared-vos"/);
});
