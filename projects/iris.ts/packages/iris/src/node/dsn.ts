import type { IrisBindings, MemorySessionBinding } from "../bindings.ts";
import { IrisFacadeError } from "../types/errors.ts";

/**
 * Open the adapter a DSN selects (internal).
 *
 * The generated client never names a backend: `createClient({ dsn })` calls
 * this, and the DSN scheme picks the session. Accepted shapes:
 *
 * - `memory:` — reference profile (conformance and internals only)
 * - `sqlite:<path>` / `sqlite::memory:`
 * - `file:<path>.yydb` / `file:<path>.yydx`
 * - `mysql://user@host:port`
 * - `postgres://user@host:port/database?sslmode=disable`
 */
export function openSessionFromDsn(core: IrisBindings, dsn: string): MemorySessionBinding {
    if (typeof dsn !== "string" || !dsn.trim()) {
        throw new IrisFacadeError("IRIS-INVALID-ARGUMENT", "`dsn` must be a non-empty string");
    }
    const text = dsn.trim();
    try {
        if (text === "memory:" || text === "memory") {
            return core.openMemorySession();
        }
        if (text.startsWith("sqlite:")) {
            return core.openSqliteSession!(text.slice("sqlite:".length) || ":memory:");
        }
        if (text.startsWith("file:")) {
            const path = text.slice("file:".length);
            if (!path) {
                throw new IrisFacadeError("IRIS-INVALID-ARGUMENT", "`file:` DSN requires a path");
            }
            return core.openYydbSession!(path);
        }
        if (text.startsWith("mysql://")) {
            return core.openMysqlSession!(text);
        }
        if (text.startsWith("postgres://") || text.startsWith("postgresql://")) {
            return core.openPostgresSession!(text);
        }
    } catch (error) {
        if (error instanceof IrisFacadeError) {
            throw error;
        }
        throw new IrisFacadeError(
            "IRIS-CONNECTION-FAILED",
            error instanceof Error ? error.message : String(error),
        );
    }
    throw new IrisFacadeError("IRIS-INVALID-ARGUMENT", `unrecognized DSN \`${text}\``);
}
