/**
 * Schema fingerprint helpers (W1).
 * Fingerprint is SHA-256 hex over UTF-8 canonical payload bytes.
 */

export type SchemaFingerprintInput = {
    /** Logical schema / contract id (e.g. project schema name). */
    schemaId: string;
    /** VOS contract / GenerationModel version string. */
    contractVersion: string;
    /**
     * Canonical schema bytes — caller supplies already-normalized VOS/contract
     * text or binary. Do not pass unstable pretty-print variants.
     */
    canonical: string | Uint8Array;
    mappingVersion?: string;
};

export type SchemaCatalogRecord = {
    schemaId: string;
    fingerprint: string;
    contractVersion: string;
    mappingVersion?: string;
    /** ISO-8601 */
    installedAt: string;
    source: "install" | "pull" | "migrate";
    /** Optional short note (never full schema dump). */
    note?: string;
};

function toBytes(canonical: string | Uint8Array): Uint8Array {
    if (typeof canonical === "string") {
        return new TextEncoder().encode(canonical);
    }
    return canonical;
}

function bytesToHex(bytes: ArrayBuffer | Uint8Array): string {
    const view = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
    let out = "";
    for (let i = 0; i < view.byteLength; i += 1) {
        out += view[i]!.toString(16).padStart(2, "0");
    }
    return out;
}

/** Compute schema fingerprint (sha-256 hex). */
export async function computeSchemaFingerprint(canonical: string | Uint8Array): Promise<string> {
    if (typeof crypto === "undefined" || !crypto.subtle) {
        throw new Error("@yydb/iris-adapter-web: Web Crypto subtle is required for fingerprints");
    }
    const digest = await crypto.subtle.digest("SHA-256", toBytes(canonical) as BufferSource);
    return bytesToHex(digest);
}

/**
 * Fingerprint envelope: includes schemaId + contractVersion so two schemas with
 * identical body but different ids do not collide in audit keys.
 */
export async function computeEnvelopeFingerprint(input: SchemaFingerprintInput): Promise<string> {
    const body = typeof input.canonical === "string" ? input.canonical : new TextDecoder().decode(input.canonical);
    const envelope = ["iris-web-schema-v1", input.schemaId, input.contractVersion, input.mappingVersion ?? "", body].join("\n");
    return computeSchemaFingerprint(envelope);
}
