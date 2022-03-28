/** Row payload from a successful VOS DML execution (binding wire shape). */
export type ExecutionRow = Record<string, string | number | boolean | null>;

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
