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

import { decodeBase64 } from "./base64.ts";

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

function decodeBase64OrThrow(text: string, field: string): Uint8Array {
    const bytes = decodeBase64(text);
    if (bytes === null) {
        throw carrierError(field, "bytes", "received malformed base64 text");
    }
    return bytes;
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
    if (typeof value === "string") {
        return decodeBase64OrThrow(value, field);
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
 * `bytes` accepts base64 text, `ArrayBuffer` and views.
 */

const KNOWN_VOS_LABELS: ReadonlySet<string> = new Set([
    "bool",
    "i8", "i16", "i32", "u8", "u16", "u32",
    "f32", "f64",
    "i64", "u64",
    "utf8", "utf16", "uuid", "decimal", "date", "time", "datetimeUtc",
    "bytes",
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
        default:
            if (!KNOWN_VOS_LABELS.has(carrier)) {
                throw unknownLabelError(carrier, field);
            }
            return normalizeWireScalar(value);
    }
}
