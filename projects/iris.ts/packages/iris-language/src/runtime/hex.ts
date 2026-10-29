/**
 * Lowercase hex codec for the tagged `bytes` carrier.
 *
 * YYDB's canonical tagged wire spells `bytes` as
 * `{"$yydb":"bytes","hex":"<lowercase hex>"}`, so parameter encoding and row
 * decoding share this single codec. Encoding always emits lowercase hex and
 * decoding rejects anything that is not, instead of accepting a second byte
 * shape.
 */

/** Encode bytes as canonical lowercase hex. */
export function encodeHex(bytes: Uint8Array): string {
    let out = "";
    for (const byte of bytes) {
        out += byte.toString(16).padStart(2, "0");
    }
    return out;
}

/** Decode canonical lowercase hex, or `null` when the text is not canonical. */
export function decodeHex(text: string): Uint8Array | null {
    if (text.length % 2 !== 0 || !/^[0-9a-f]*$/.test(text)) {
        return null;
    }
    const bytes = new Uint8Array(text.length / 2);
    for (let index = 0; index < bytes.length; index += 1) {
        bytes[index] = Number.parseInt(text.slice(index * 2, index * 2 + 2), 16);
    }
    return bytes;
}
