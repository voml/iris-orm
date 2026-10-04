import type { ExecutionRow } from "../types/execution-result.ts";

/** Parse `Entity.operation` operation ids into entity names. */
export function entityFromOperationId(operationId: string): string | null {
    const dot = operationId.indexOf(".");
    if (dot <= 0) {
        return null;
    }
    return operationId.slice(0, dot);
}

/** Map one SQLite/D1 wire row into TypeScript author field names. */
export function wireRowToAuthorRow(
    wireToTs: Readonly<Record<string, string>>,
    row: Record<string, unknown>,
): ExecutionRow {
    const mapped: ExecutionRow = {};
    for (const [tsName, wireName] of Object.entries(wireToTs)) {
        if (wireName in row) {
            const value = row[wireName];
            mapped[tsName] =
                typeof value === "string" || typeof value === "number" || typeof value === "boolean" || value === null
                    ? value
                    : value == null
                      ? null
                      : String(value);
        }
    }
    return mapped;
}

/** Map wire rows when a generated-client wire table is available. */
export function mapRowsToAuthorSurface(
    rows: readonly ExecutionRow[],
    wireToTs: Readonly<Record<string, string>>,
): readonly ExecutionRow[] {
    if (Object.keys(wireToTs).length === 0) {
        return rows;
    }
    return rows.map((row) => wireRowToAuthorRow(wireToTs, row as Record<string, unknown>));
}
