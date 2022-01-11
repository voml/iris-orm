/**
 * `@yydb/iris-types` — native TypeScript types layer.
 *
 * Mirrors Rust `iris-types` (session / planner / capability / runtime) without
 * sharing ABI. Do not import Rust crates or N-API bindings here.
 */

/** Marker type until the native runtime is implemented. */
export type IrisPlaceholder = {
    readonly __irisTypes: "native-ts-skeleton";
};
