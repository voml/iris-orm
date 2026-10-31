/**
 * Wire value → generated TypeScript carrier conversion.
 *
 * Carriers come from the generated `IRIS_VALUE_CARRIERS` table, which mirrors
 * the VOS value mapping (`i64` / `u64` → `bigint`, `bytes` → `Uint8Array`).
 * Fields whose carrier stays a JS scalar pass through unchanged. A value that
 * cannot be converted throws: the generated type promise must not be satisfied
 * by silently stringifying it.
 */

import type { WireCarrierValue } from "./host.ts";

import { decodeHex } from "./hex.ts";

function carrierError(field: string, carrier: string, detail: string): Error {
    return new Error(`IRIS-VALUE-CARRIER: \`${field}\` promised VOS \`${carrier}\`: ${detail}`);
}

/** Wire scalars surfaced as-is; only non-scalar leftovers are stringified. */
function normalizeWireScalar(value: unknown): WireCarrierValue {
    if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
        return value;
    }
    return value == null ? null : String(value);
}

function taggedBytesHex(value: unknown, field: string): string | null {
    if (typeof value !== "object" || value === null) {
        return null;
    }
    const record = value as Record<string, unknown>;
    if (record["$yydb"] !== "bytes") {
        return null;
    }
    const hex = record["hex"];
    if (typeof hex !== "string") {
        throw carrierError(field, "bytes", "received tagged `bytes` without a `hex` string");
    }
    return hex;
}

function toExactBigInt(value: unknown, field: string, carrier: string): bigint {
    if (typeof value === "bigint") {
        return value;
    }
    if (typeof value === "number") {
        if (!Number.isSafeInteger(value)) {
            throw carrierError(field, carrier, `received number ${value} beyond the 2^53 exact range, bind an exact string instead`);
        }
        return BigInt(value);
    }
    if (typeof value === "string") {
        const text = value.trim();
        if (/^[+-]?[0-9]+$/.test(text)) {
            return BigInt(text);
        }
    }
    throw carrierError(field, carrier, `cannot convert ${typeof value} value to bigint`);
}

/** Tagged payload member of a canonical `$yydb` carrier object. */
function taggedMember(value: unknown, tag: string, field: string, carrier: string): Record<string, unknown> {
    if (typeof value !== "object" || value === null) {
        throw carrierError(field, carrier, `received a value that is not the tagged \`${tag}\` carrier`);
    }
    const record = value as Record<string, unknown>;
    if (record["$yydb"] !== tag) {
        throw carrierError(field, carrier, `received an object that is not the tagged \`${tag}\` carrier`);
    }
    return record;
}

function toFloat32Vector(value: unknown, field: string): Float32Array {
    if (value instanceof Float32Array) {
        return new Float32Array(value);
    }
    const carrier = taggedMember(value, "vector", field, "vector");
    const dimension = carrier["dimension"];
    if (typeof dimension !== "number" || !Number.isSafeInteger(dimension) || dimension <= 0) {
        throw carrierError(field, "vector", `received tagged \`vector\` with a non-positive \`dimension\`: ${String(dimension)}`);
    }
    const hex = carrier["components"];
    if (typeof hex !== "string") {
        throw carrierError(field, "vector", "received tagged `vector` without a `components` hex string");
    }
    const octets = decodeHex(hex);
    if (octets === null) {
        throw carrierError(field, "vector", "received tagged `vector` without canonical lowercase hex");
    }
    if (octets.byteLength !== dimension * 4) {
        throw carrierError(field, "vector", `received tagged \`vector\` with ${octets.byteLength} octets for dimension ${dimension}`);
    }
    const view = new DataView(octets.buffer as ArrayBuffer, octets.byteOffset, octets.byteLength);
    const components = new Float32Array(dimension);
    for (let index = 0; index < dimension; index += 1) {
        components[index] = view.getFloat32(index * 4, true);
    }
    return components;
}

function toFileDescriptor(value: unknown, field: string): WireCarrierValue {
    const carrier = taggedMember(value, "file", field, "file");
    const digest = carrier["digest"];
    if (typeof digest !== "string") {
        throw carrierError(field, "file", "received tagged `file` without a `digest` string");
    }
    const manifest = carrier["manifest"];
    if (typeof manifest !== "object" || manifest === null) {
        throw carrierError(field, "file", "received tagged `file` without a `manifest` object");
    }
    const manifestRecord = manifest as Record<string, unknown>;
    const hash = manifestRecord["hash"];
    if (typeof hash !== "string") {
        throw carrierError(field, "file", "received tagged `file` whose `manifest.hash` is not a string");
    }
    if (carrier["octets"] === undefined || carrier["octets"] === null) {
        throw carrierError(field, "file", "received tagged `file` without an `octets` number");
    }
    if (manifestRecord["size"] === undefined || manifestRecord["size"] === null) {
        throw carrierError(field, "file", "received tagged `file` whose `manifest.size` is missing");
    }
    const label = (member: string): string | undefined => {
        const entry = carrier[member];
        if (entry === undefined || entry === null) {
            return undefined;
        }
        if (typeof entry !== "string" || entry.trim() === "") {
            throw carrierError(field, "file", `received tagged \`file\` whose \`${member}\` is not a non-blank string`);
        }
        return entry;
    };
    const descriptor: {
        readonly digest: string;
        readonly octets: bigint;
        readonly name?: string;
        readonly mediaType?: string;
        readonly manifest: { readonly hash: string; readonly size: bigint };
    } = {
        digest,
        octets: toExactBigInt(carrier["octets"], field, "file"),
        manifest: { hash, size: toExactBigInt(manifestRecord["size"], field, "file") },
    };
    const name = label("name");
    if (name !== undefined) {
        (descriptor as { name?: string }).name = name;
    }
    const mediaType = label("mediaType");
    if (mediaType !== undefined) {
        (descriptor as { mediaType?: string }).mediaType = mediaType;
    }
    return descriptor;
}

function toUint8Array(value: unknown, field: string): Uint8Array {
    if (value instanceof Uint8Array) {
        return new Uint8Array(value);
    }
    if (value instanceof ArrayBuffer) {
        return new Uint8Array(value.slice(0));
    }
    if (ArrayBuffer.isView(value)) {
        const view = value as ArrayBufferView;
        const copy = new Uint8Array(view.byteLength);
        copy.set(new Uint8Array(view.buffer as ArrayBuffer, view.byteOffset, view.byteLength));
        return copy;
    }
    if (typeof value === "object" && value !== null && !(value instanceof ArrayBuffer) && !ArrayBuffer.isView(value)) {
        const hex = taggedBytesHex(value, field);
        if (hex === null) {
            throw carrierError(field, "bytes", "received an object that is not the tagged `bytes` carrier");
        }
        const bytes = decodeHex(hex);
        if (bytes === null) {
            throw carrierError(field, "bytes", "received tagged `bytes` without canonical lowercase hex");
        }
        return bytes;
    }
    throw carrierError(field, "bytes", `cannot convert ${typeof value} value to Uint8Array`);
}

/**
 * Convert one wire value to the carrier the generated type promises.
 *
 * `carrier` is the VOS base label from `IRIS_VALUE_CARRIERS` (`i64`, `u64`,
 * `bytes`, …). Every label must be in the faithful VOS carrier table — unknown
 * labels hard-fail instead of falling back to `unknown`. `i64` accepts exact
 * decimal strings (the wire shape for values past 2^53) or safe numbers.
 * `bytes` accepts the tagged `{"$yydb":"bytes","hex":…}` value, `ArrayBuffer`
 * and views. `vector` accepts the tagged
 * `{"$yydb":"vector","dimension":…,"components":…}` value (little-endian
 * `f32` hex) and yields a `Float32Array`. `file` accepts the tagged
 * `{"$yydb":"file",…}` descriptor and yields the inline file carrier shape.
 */

const KNOWN_VOS_LABELS: ReadonlySet<string> = new Set([
    "bool",
    "i8", "i16", "i32", "u8", "u16", "u32",
    "f32", "f64",
    "i64", "u64",
    "utf8", "utf16", "uuid", "decimal", "date", "time", "datetimeUtc",
    "bytes", "vector", "file",
    "unit",
]);

function unknownLabelError(label: string, field: string): Error {
    const known = [...KNOWN_VOS_LABELS].sort().join(", ");
    return new Error(
        `IRIS-VALUE-CARRIER: field \`${field}\` carries unknown VOS label \`${label}\`. Known labels: ${known}`,
    );
}

export function decodeWireValueByCarrier(carrier: string | undefined, value: unknown, field = "value"): WireCarrierValue {
    if (value === null || value === undefined) {
        return null;
    }
    if (carrier === undefined) {
        return normalizeWireScalar(value);
    }
    switch (carrier) {
        case "i64":
        case "u64":
            return toExactBigInt(value, field, carrier);
        case "bytes":
            return toUint8Array(value, field);
        case "vector":
            return toFloat32Vector(value, field);
        case "file":
            return toFileDescriptor(value, field);
        default:
            if (!KNOWN_VOS_LABELS.has(carrier)) {
                throw unknownLabelError(carrier, field);
            }
            return normalizeWireScalar(value);
    }
}
