/** Target-native error surface (shim until full IrisError mapping). */
export type IrisGeneratedError = {
    /** Stable author-facing code (`IRIS-UNIQUE-CONFLICT`, …). */
    code: string;
    /** Semantic class (`unique_conflict`, `not_found`, …). */
    kind?: string;
    /** Internal backend / host code, for diagnostics only. */
    diagnosticCode?: string;
    message: string;
    path?: string;
    span?: { start: number; end: number };
    cause?: unknown;
};
