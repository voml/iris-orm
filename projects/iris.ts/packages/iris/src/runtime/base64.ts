/**
 * Base64 codec for the `bytes` carrier on the JSON wire.
 *
 * `i64` and `bytes` are the two carriers that cannot cross a JSON boundary
 * natively: `i64` is sent as exact decimal digits and `bytes` as base64 text.
 * Both sides of the wire (parameter encoding and row decoding) must use this
 * single codec so a round trip stays byte-identical.
 */

const BASE64_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/** Encode bytes as base64 text (no padding variant on decode). */
export function encodeBase64(bytes: Uint8Array): string {
    let out = "";
    for (let index = 0; index < bytes.length; index += 3) {
        const first = bytes[index]!;
        const second = bytes[index + 1];
        const third = bytes[index + 2];
        out += BASE64_ALPHABET[first >> 2];
        out += BASE64_ALPHABET[((first & 0x03) << 4) | ((second ?? 0) >> 4)];
        out += second === undefined ? "=" : BASE64_ALPHABET[((second & 0x0f) << 2) | ((third ?? 0) >> 6)];
        out += third === undefined ? "=" : BASE64_ALPHABET[third & 0x3f];
    }
    return out;
}

/** Decode base64 text, or `null` when the text is not valid base64. */
export function decodeBase64(text: string): Uint8Array | null {
    const clean = text.replace(/[\s=]/g, "");
    if (clean.length % 4 === 1) {
        return null;
    }
    const bytes = new Uint8Array((clean.length * 3) >> 2);
    let buffer = 0;
    let bits = 0;
    let written = 0;
    for (const char of clean) {
        const value = BASE64_ALPHABET.indexOf(char);
        if (value < 0) {
            return null;
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
