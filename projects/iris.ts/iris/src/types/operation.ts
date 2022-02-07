/** Scalar wire values for structured Iris operations. */
export type IrisScalar = string | number | boolean | null;

/** Single-field equality filter (Phase 1 operation ABI). */
export interface IrisWhereEq {
    field: string;
    value: IrisScalar;
}

/** Stable structured operation passed from generated client to runtime ABI. */
export type IrisOperation =
    | {
          kind: "find-many";
          entity: string;
          where?: IrisWhereEq;
          take?: number;
      }
    | {
          kind: "find-unique";
          entity: string;
          where: IrisWhereEq;
      };
