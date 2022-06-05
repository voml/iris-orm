import type { CreateIrisDbBindingOptions, IrisDbBinding } from "../types/executor.ts";
import type { OperationExecutor } from "../types/operation-executor.ts";
import {
    createIrisDbBinding as createBrowserBinding,
    createIrisOperationExecutor as createBrowserOperationExecutor,
} from "../browser/executor.ts";

/** Symmetric generated-client binding factory for `@yydb/iris/wasm` (same name as `@yydb/iris/node`). */
export async function createIrisDbBinding(options: CreateIrisDbBindingOptions = {}): Promise<IrisDbBinding> {
    return createBrowserBinding(options);
}

/** Symmetric operation executor factory for `@yydb/iris/wasm`. */
export async function createIrisOperationExecutor(options: CreateIrisDbBindingOptions = {}): Promise<OperationExecutor> {
    return createBrowserOperationExecutor(options);
}
