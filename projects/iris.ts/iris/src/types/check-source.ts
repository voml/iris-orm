/** Binding-neutral result of validating a VOS / `.iris` schema source. */
export interface CheckSourceResult {
    ok: boolean;
    tableCount: number;
    schemaFingerprint: string;
    generatorVersion: string;
    error?: string | null;
}
