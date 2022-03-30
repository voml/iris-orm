import type { CreateIrisDbBindingOptions, IrisDbBinding } from "../types/executor.ts";
import { mapWireToQueryValue } from "../types/executor.ts";
import { parseExecuteJson } from "../runtime/parse.ts";
import { getWasmSemanticCore } from "./wasm.ts";

/**
 * Create internal binding support for generated browser `db` (not an application entry).
 *
 * Requires prior `initIris()`. Browser host currently uses WASM in-memory ReferenceStore only.
 */
export async function createBrowserIrisDbBinding(_options: CreateIrisDbBindingOptions = {}): Promise<IrisDbBinding> {
    const session = getWasmSemanticCore().openMemorySession();

    return {
        async query(source: string, parameters?: Readonly<Record<string, unknown>>): Promise<unknown> {
            if (parameters !== undefined && Object.keys(parameters).length > 0) {
                throw new Error("@yydb/iris: browser WASM binding does not support VOS parameters yet");
            }
            const parsed = parseExecuteJson(session.executeVos(source));
            if (!parsed.ok) {
                throw new Error(parsed.error ?? "iris execution failed");
            }
            return mapWireToQueryValue({ kind: "rows", rows: parsed.rows });
        },
        async execute(source: string, parameters?: Readonly<Record<string, unknown>>): Promise<void> {
            await this.query(source, parameters);
        },
        async close() {
            session.close();
        },
    };
}
