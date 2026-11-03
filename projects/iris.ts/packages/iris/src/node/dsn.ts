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
        throw new IrisFacadeError(...decodeEnvelopeError(error));
    }
}

/** Decode a native `{code, message}` payload, falling back to connection failure. */
function decodeEnvelopeError(error: unknown): [code: string, message: string] {
    const message = error instanceof Error ? error.message : String(error);
    try {
        const payload = JSON.parse(message) as { code?: unknown; message?: unknown };
        if (typeof payload.code === "string" && typeof payload.message === "string") {
            return [payload.code, payload.message];
        }
    } catch {
        // not an envelope payload
    }
    return ["IRIS-CONNECTION-FAILED", message];
}
