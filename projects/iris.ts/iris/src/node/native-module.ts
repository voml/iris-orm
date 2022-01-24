/** N-API exports from optional platform packages (`@yydb/iris-win32-x64`, …). */
export type IrisNativeModule = {
    irisVersion(): string;
    checkSource(source: string): IrisNativeCheckResult;
};

export type IrisNativeCheckResult = {
    ok: boolean;
    tableCount: number;
    schemaFingerprint: string;
    generatorVersion: string;
    error?: string | null;
};
