import type { SchemaIntrospection } from "../types/schema-introspection.ts";
import type { ExecuteResult, IrisRow } from "../types/session.ts";

/** Parse execute JSON from Rust bindings. */
export function parseExecuteJson(raw: string): ExecuteResult {
    const parsed = JSON.parse(raw) as {
        ok?: boolean;
        rows?: IrisRow[];
        error?: string | null;
    };
    return {
        ok: Boolean(parsed.ok),
        rows: Array.isArray(parsed.rows) ? parsed.rows : [],
        error: parsed.error ?? null,
    };
}

/** Parse introspection JSON from Rust bindings. */
export function parseIntrospectionJson(raw: string): SchemaIntrospection {
    return JSON.parse(raw) as SchemaIntrospection;
}

/** Parse N-API `rows_json` payload. */
export function parseRowsJson(raw: string): IrisRow[] {
    const parsed = JSON.parse(raw) as unknown;
    return Array.isArray(parsed) ? (parsed as IrisRow[]) : [];
}
