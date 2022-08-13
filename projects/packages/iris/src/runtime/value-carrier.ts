/**
 * Wire value → generated TypeScript carrier conversion.
 *
 * Carriers come from the generated `IRIS_VALUE_CARRIERS` table, which mirrors
 * the VOS value mapping (`i64` / `u64` → `bigint`, `bytes` → `Uint8Array`).
 * Fields whose carrier stays a JS scalar pass through unchanged. A value that
 * cannot be converted throws: the generated type promise must not be satisfied
 * by silently stringifying it.
 */

import type { WireCarrierValue } from "../types/execution-result.ts";

const BASE64_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

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

function decodeBase64(text: string, field: string): Uint8Array {
    const clean = text.replace(/[\s=]/g, "");
    if (clean.length % 4 === 1) {
        throw carrierError(field, "bytes", "received malformed base64 text");
    }
    const bytes = new Uint8Array((clean.length * 3) >> 2);
    let buffer = 0;
    let bits = 0;
    let written = 0;
    for (const char of clean) {
        const value = BASE64_ALPHABET.indexOf(char);
        if (value < 0) {
            throw carrierError(field, "bytes", "received malformed base64 text");
        }
        buffer = (buffer << 6) | value;
        bits += 6;
        if (bits >= 8) {
            bits -= 8;
            bytes[written] = (buffer >> bits) & 0xff;
            written += 1;
        }
    }
    return bytes.slice(0, written);
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
        return decodeBase64(value, field);
    }
    throw carrierError(field, "bytes", `cannot convert ${typeof value} value to Uint8Array`);
}

/**
 * Convert one wire value to the carrier the generated type promises.
 *
 * `carrier` is the VOS base label from `IRIS_VALUE_CARRIERS` (`i64`, `u64`,
 * `bytes`, …). Labels without a distinct JS carrier pass through; note that no
 * fixture covers `bytes` yet (the SQLite/JSON wire paths both hand back text
 * for that column until a binary provider fixture exists).
 */
export function decodeWireValueByCarrier(carrier: string | undefined, value: unknown, field = "value"): WireCarrierValue {
    if (value === null || value === undefined) {
        return null;
    }
    switch (carrier) {
        case "i64":
        case "u64":
            return toExactBigInt(value, field, carrier);
        case "bytes":
            return toUint8Array(value, field);
        default:
            return normalizeWireScalar(value);
    }
}
