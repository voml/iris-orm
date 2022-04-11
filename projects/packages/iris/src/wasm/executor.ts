import type { CreateIrisDbBindingOptions, IrisDbBinding } from "../types/executor.ts";
import { createIrisDbBinding as createBrowserBinding } from "../browser/executor.ts";

/** Symmetric generated-client binding factory for `@yydb/iris/wasm` (same name as `@yydb/iris/node`). */
export async function createIrisDbBinding(options: CreateIrisDbBindingOptions = {}): Promise<IrisDbBinding> {
    return createBrowserBinding(options);
}
