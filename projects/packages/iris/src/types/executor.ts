import type { ExecutionWireResult } from "./execution-result.ts";

/** VOS parameter bindings for direct text execution. */
export type VosParameters = Readonly<Record<string, unknown>>;

/**
 * Internal runtime support consumed by generated `db` (not an application entry).
 *
 * Generated code synthesizes VOS text; Rust owns parse, validation, planning,
 * binary encoding, and execution.
 */
export interface IrisDbBinding {
    /** DML entry: returns the VOS operation value (host-mapped). */
    query(source: string, parameters?: VosParameters): Promise<unknown>;
    /** DDL entry: returns VOS unit (mapped to void in TypeScript). */
    execute(source: string, parameters?: VosParameters): Promise<void>;
    close(): Promise<void>;
}

/** Generated-client wiring options (host-only; users import `db` from `./generated`). */
export interface CreateIrisDbBindingOptions {
    profile?: "memory" | "sqlite" | "project";
    sqlitePath?: string;
    project?: string;
    source?: string;
    /** Apply managed-push schema after opening a SQLite session. */
    schema?: string;
}

/** @deprecated Use `IrisDbBinding`. */
export type IrisExecutor = IrisDbBinding;

/** @deprecated Use `CreateIrisDbBindingOptions`. */
export type CreateIrisExecutorOptions = CreateIrisDbBindingOptions;

/** Map binding wire shape to the VOS value surfaced by db.$query. */
export function mapWireToQueryValue(wire: ExecutionWireResult): unknown {
    switch (wire.kind) {
        case "rows":
            return wire.rows;
        case "value":
            return wire.value;
        case "affected":
            return wire.affected;
        case "unit":
            return undefined;
    }
}
