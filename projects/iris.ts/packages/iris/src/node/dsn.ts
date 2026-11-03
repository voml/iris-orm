import type { IrisBindings, MemorySessionBinding } from "../bindings.ts";
import { IrisFacadeError } from "../types/errors.ts";

/**
 * Open the adapter a DSN selects (internal).
 *
 * Parsing and adapter selection live in `iris-host`, so Node, Python and the
 * browser cannot drift apart. `createClient({ dsn })` calls this and the
 * generated schema travels with it; managed adapters apply it on open.
 */
export function openSessionFromDsn(
    core: IrisBindings,
    dsn: string,
    schemaSource?: string,
): MemorySessionBinding {
    try {
        return core.openDsnSession!(dsn, schemaSource && schemaSource.trim() ? schemaSource : null);
    } catch (error) {
        const decoded = decodeEnvelopeError(error);
        throw new IrisFacadeError(decoded.code, decoded.message, {
            kind: decoded.kind,
            diagnosticCode: decoded.diagnosticCode,
        });
    }
}

/** Decode a native error payload, falling back to connection failure. */
function decodeEnvelopeError(error: unknown): {
    code: string;
    message: string;
    kind?: string;
    diagnosticCode?: string;
} {
    const message = error instanceof Error ? error.message : String(error);
    try {
        const payload = JSON.parse(message) as {
            code?: unknown;
            message?: unknown;
            kind?: unknown;
            diagnosticCode?: unknown;
        };
        if (typeof payload.code === "string" && typeof payload.message === "string") {
            return {
                code: payload.code,
                message: payload.message,
                kind: typeof payload.kind === "string" ? payload.kind : undefined,
                diagnosticCode: typeof payload.diagnosticCode === "string" ? payload.diagnosticCode : undefined,
            };
        }
    } catch {
        // not an envelope payload
    }
    return { code: "IRIS-CONNECTION-FAILED", message };
}
