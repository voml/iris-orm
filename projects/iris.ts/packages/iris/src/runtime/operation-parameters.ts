/**
 * Operation-parameter encoding for the JSON transport (N-API / WASM hosts).
 *
 * VOS parameters are bound by the Rust authority (`bind_parameters`), so the
 * request has to cross a JSON boundary first. Two carriers cannot do that
 * natively:
 *
 * - `i64` / `u64` author values are `bigint`; JSON has no bigint, and
 *   `JSON.stringify` throws on it. They are emitted as exact decimal digit
 *   tokens so the Rust side keeps every bit.
 * - `bytes` author values are `Uint8Array`; `JSON.stringify` would turn them
 *   into `{"0":…}` objects, which the binder rejects. They are emitted as the
 *   canonical tagged `{"$yydb":"bytes","hex":"<lowercase hex>"}` value, which
 *   a typed-value backend binds directly instead of reading as text.
 * - `vector` author values are `Float32Array`; they are emitted as the
 *   canonical tagged `{"$yydb":"vector","dimension":…,"components":…}` value
 *   with little-endian `f32` hex.
 * - `file` author values are the inline descriptor shape
 *   (`{digest, octets, name?, mediaType?, manifest}`); they are emitted as
 *   the canonical tagged `{"$yydb":"file",…}` value.
 *
 * Anything else is a scalar VOS literal (`null`, `bool`, number, string).
 * Nested objects are rejected instead of being silently stringified.
 */

import type { OperationRequest } from "./host.ts";

import { encodeHex } from "./hex.ts";

const I64_MIN = -(2n ** 63n);
const I64_MAX = 2n ** 63n - 1n;

/** A parameter value the JSON transport cannot carry without losing its carrier type. */
export class IrisParameterEncodingError extends Error {
    readonly code = "IRIS-PARAM-ENCODING";

    constructor(detail: string) {
        super(`IRIS-PARAM-ENCODING: ${detail}`);
        this.name = "IrisParameterEncodingError";
    }
}

/** Exact decimal digits of an `i64`-shaped `bigint` parameter. */
export function exactI64Text(value: bigint, subject: string): string {
    if (value < I64_MIN || value > I64_MAX) {
        throw new IrisParameterEncodingError(
            `${subject} is outside the VOS \`i64\` range (${I64_MIN}..${I64_MAX}) and cannot be bound without losing precision`,
        );
    }
    return value.toString();
}

function toBytes(value: ArrayBuffer | ArrayBufferView): Uint8Array {
    if (value instanceof ArrayBuffer) {
        return new Uint8Array(value.slice(0));
    }
    const copy = new Uint8Array(value.byteLength);
    copy.set(new Uint8Array(value.buffer as ArrayBuffer, value.byteOffset, value.byteLength));
    return copy;
}

function isBytes(value: unknown): value is ArrayBuffer | ArrayBufferView {
    return value instanceof ArrayBuffer || ArrayBuffer.isView(value);
}

/** Exact non-negative integer text of a `u64`-shaped wire number member. */
function u64Token(value: unknown, subject: string): string {
    if (typeof value === "bigint") {
        if (value < 0n) {
            throw new IrisParameterEncodingError(`${subject} is negative and outside the VOS \`u64\` range`);
        }
        return value.toString();
    }
    if (typeof value === "number") {
        if (!Number.isSafeInteger(value) || value < 0) {
            throw new IrisParameterEncodingError(
                `${subject} is ${value}, outside the exact non-negative integer range the \`u64\` wire number carries`,
            );
        }
        return String(value);
    }
    throw new IrisParameterEncodingError(`${subject} must be a number or bigint, not ${typeof value}`);
}

function encodeVectorCarrier(name: string, value: Float32Array): string {
    for (const component of value) {
        if (!Number.isFinite(component)) {
            throw new IrisParameterEncodingError(`parameter \`${name}\` is a \`Float32Array\` with a non-finite component`);
        }
    }
    const copy = new Float32Array(value.length);
    copy.set(value);
    const octets = new Uint8Array(copy.buffer as ArrayBuffer);
    const dimension = u64Token(copy.length, `parameter \`${name}\` dimension`);
    return `{"$yydb":"vector","dimension":${dimension},"components":${JSON.stringify(encodeHex(octets))}}`;
}

function fileCarrierMembers(value: object): Record<string, unknown> | null {
    const record = value as Record<string, unknown>;
    const octets = record["octets"];
    if (typeof record["digest"] !== "string" || (typeof octets !== "number" && typeof octets !== "bigint")) {
        return null;
    }
    const manifest = record["manifest"];
    if (typeof manifest !== "object" || manifest === null) {
        return null;
    }
    if (typeof (manifest as Record<string, unknown>)["hash"] !== "string") {
        return null;
    }
    return record;
}

function encodeFileCarrier(name: string, record: Record<string, unknown>): string {
    const subject = `parameter \`${name}\``;
    const octets = u64Token(record["octets"], `${subject} \`octets\``);
    const label = (member: string): string | null => {
        const entry = record[member];
        if (entry === undefined || entry === null) {
            return null;
        }
        if (typeof entry !== "string" || entry.trim() === "") {
            throw new IrisParameterEncodingError(`${subject} \`${member}\` must be a non-blank string when present`);
        }
        return JSON.stringify(entry);
    };
    const manifest = record["manifest"] as Record<string, unknown>;
    const manifestText = `{"hash":${JSON.stringify(manifest["hash"])},"size":${u64Token(manifest["size"], `${subject} \`manifest.size\``)}}`;
    const parts = [`"$yydb":"file"`, `"digest":${JSON.stringify(record["digest"])}`, `"octets":${octets}`];
    const labelName = label("name");
    if (labelName !== null) {
        parts.push(`"name":${labelName}`);
    }
    const labelMediaType = label("mediaType");
    if (labelMediaType !== null) {
        parts.push(`"mediaType":${labelMediaType}`);
    }
    parts.push(`"manifest":${manifestText}`);
    return `{${parts.join(",")}}`;
}

function encodeParameterValue(name: string, value: unknown): string | undefined {
    if (typeof value === "bigint") {
        return exactI64Text(value, `parameter \`${name}\``);
    }
    if (value instanceof Float32Array) {
        return encodeVectorCarrier(name, value);
    }
    if (isBytes(value)) {
        return JSON.stringify({ $yydb: "bytes", hex: encodeHex(toBytes(value)) });
    }
    if (typeof value === "object" && value !== null) {
        const record = fileCarrierMembers(value);
        if (record !== null) {
            return encodeFileCarrier(name, record);
        }
    }
    if (typeof value === "number") {
        if (!Number.isFinite(value)) {
            throw new IrisParameterEncodingError(`parameter \`${name}\` is not a finite number`);
        }
        return JSON.stringify(value);
    }
    if (typeof value === "string" || typeof value === "boolean") {
        return JSON.stringify(value);
    }
    if (value === null) {
        return "null";
    }
    if (value === undefined) {
        return undefined;
    }
    throw new IrisParameterEncodingError(
        `parameter \`${name}\` must be a scalar (\`null\`, boolean, number, string), an exact \`bigint\`, \`Uint8Array\`, \`Float32Array\` or a file descriptor object`,
    );
}

/**
 * JSON object text of an operation parameter record.
 *
 * `bigint` stays an unquoted exact integer token and `Uint8Array` becomes the
 * canonical tagged `bytes` value. `undefined` entries are dropped, matching
 * `JSON.stringify`.
 */
export function stringifyOperationParameters(parameters?: Readonly<Record<string, unknown>>): string {
    if (parameters === undefined) {
        return "{}";
    }
    const entries: string[] = [];
    for (const [name, value] of Object.entries(parameters)) {
        const encoded = encodeParameterValue(name, value);
        if (encoded !== undefined) {
            entries.push(`${JSON.stringify(name)}:${encoded}`);
        }
    }
    return `{${entries.join(",")}}`;
}

/** JSON text of an operation request whose parameters keep `bigint` / `bytes` carriers. */
export function stringifyOperationRequest(request: OperationRequest): string {
    const { parameters, ...rest } = request;
    const head = JSON.stringify(rest);
    if (parameters === undefined) {
        return head;
    }
    return `${head.slice(0, -1)},"parameters":${stringifyOperationParameters(parameters)}}`;
}
