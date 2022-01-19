/**
 * `@yydb/iris-types` — declarations for the Rust Iris binding surface.
 *
 * Rust owns runtime semantics. This package only describes values crossing the
 * Node N-API or browser-WASM boundary and must not grow a parallel planner.
 */

/** Marker type until the native runtime is implemented. */
export type IrisPlaceholder = {
    readonly __irisTypes: "rust-binding-skeleton";
};
