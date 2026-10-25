import type { WebExecuteResult, WebIrisRow, WebSchemaIntrospection } from "@yydb/iris-unknown-wasm32";

/** Parse execute JSON from the WASM binding. */
export function parseExecuteJson(raw: string): WebExecuteResult {
    const parsed = JSON.parse(raw) as {
        ok?: boolean;
        rows?: WebIrisRow[];
        error?: string | null;
    };
    return {
        ok: Boolean(parsed.ok),
        rows: Array.isArray(parsed.rows) ? parsed.rows : [],
        error: parsed.error ?? null,
    };
}

/** Parse introspection JSON from the WASM binding. */
export function parseIntrospectionJson(raw: string): WebSchemaIntrospection {
    return JSON.parse(raw) as WebSchemaIntrospection;
}

/** Parse a `rows_json` payload. */
export function parseRowsJson(raw: string): WebIrisRow[] {
    const parsed = JSON.parse(raw) as unknown;
    return Array.isArray(parsed) ? (parsed as WebIrisRow[]) : [];
}
