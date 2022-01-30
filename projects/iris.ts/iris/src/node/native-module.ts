import type { CheckSourceResult } from "../types/check-source.ts";

/** N-API exports from optional platform packages (`@yydb/iris-win32-x64`, …). */
export type IrisNativeModule = {
    irisVersion(): string;
    checkSource(source: string): CheckSourceResult;
};
