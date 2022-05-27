import type { CreateIrisDbBindingOptions, IrisDbBinding } from "../types/executor.ts";
import { createIrisDbBindingFromSession } from "../runtime/db-binding.ts";
import { openBindingSession } from "../runtime/open-binding-session.ts";
import { getWasmSemanticCore } from "../wasm/state.ts";

/**
 * Create internal binding support for generated browser `db` (not an application entry).
 *
 * Requires prior `initIris()` / `loadIrisWeb()`. Browser host currently uses WASM in-memory ReferenceStore only.
 * Generated `browser.ts` imports the same factory from `@yydb/iris/wasm`.
 */
export async function createIrisDbBinding(options: CreateIrisDbBindingOptions = {}): Promise<IrisDbBinding> {
    const session = await openBindingSession("browser", getWasmSemanticCore(), options);
    return createIrisDbBindingFromSession(session);
}

/** @deprecated Use `createIrisDbBinding` or import from `@yydb/iris/wasm`. */
export const createBrowserIrisDbBinding = createIrisDbBinding;
