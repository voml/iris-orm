import type { ExecuteResult, IrisRow, SchemaIntrospection } from "./host.ts";

/** Parse execute JSON from the host binding. */
export function parseExecuteJson(raw: string): ExecuteResult {
    const parsed = JSON.parse(raw) as {
        ok?: boolean;
        rows?: IrisRow[];
        error?: string | null;
        errorCode?: string | null;
        errorKind?: string | null;
        errorDiagnosticCode?: string | null;
    };
    return {
        ok: Boolean(parsed.ok),
        rows: Array.isArray(parsed.rows) ? parsed.rows : [],
        error: parsed.error ?? null,
        errorCode: parsed.errorCode ?? null,
        errorKind: parsed.errorKind ?? null,
        errorDiagnosticCode: parsed.errorDiagnosticCode ?? null,
    };
}

/** Parse introspection JSON from the host binding. */
export function parseIntrospectionJson(raw: string): SchemaIntrospection {
    return JSON.parse(raw) as SchemaIntrospection;
}

/** Parse a `rows_json` payload. */
export function parseRowsJson(raw: string): IrisRow[] {
    const parsed = JSON.parse(raw) as unknown;
    return Array.isArray(parsed) ? (parsed as IrisRow[]) : [];
}
