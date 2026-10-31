/**
 * JS carrier a generated value table can promise for one field.
 *
 * `bigint` and `Uint8Array` are carriers, not conveniences: VOS `i64` must not be
 * flattened into a lossy JS `number`, and VOS `bytes` stays binary.
 * `Float32Array` carries VOS `vector` components, and the inline descriptor
 * carries VOS `file` identity without degrading to a tagged JSON string.
 */
export type WireCarrierValue =
    | string
    | number
    | boolean
    | bigint
    | Uint8Array
    | Float32Array
    | {
          readonly digest: string;
          readonly octets: bigint;
          readonly name?: string;
          readonly mediaType?: string;
          readonly manifest: { readonly hash: string; readonly size: bigint };
      }
    | null;

/**
 * Row payload from a successful VOS DML execution (binding wire shape).
 *
 * Carriers follow the generated `IRIS_VALUE_CARRIERS` table, so an `i64` column
 * is surfaced as `bigint` and a `bytes` column as `Uint8Array` instead of being
 * flattened into a JS scalar.
 */
export type ExecutionRow = Record<string, WireCarrierValue>;

/**
 * Binding wire result before host value mapping.
 *
 * Not exported as the public shape of `db.$query` / `db.$execute`.
 * `db.$query` surfaces `unknown` (VOS value); `db.$execute` surfaces `void` (VOS unit).
 */
export type ExecutionWireResult =
    | {
          kind: "rows";
          rows: readonly ExecutionRow[];
      }
    | {
          kind: "affected";
          affected: number;
      }
    | {
          kind: "value";
          value: unknown;
      }
    | {
          kind: "unit";
      };

/** @deprecated Use `ExecutionWireResult`. */
export type ExecutionResult = ExecutionWireResult;
