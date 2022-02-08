import { resolve } from "node:path";

import type { SchemaIntrospection } from "../types/schema-introspection.ts";
import { emitTypescriptClient } from "../codegen/emit-typescript.ts";
import { loadSemanticCore } from "./native.ts";

/** Generate TypeScript Iris client from schema source. */
export async function generateTypescriptClient(
    source: string,
    outDir: string,
): Promise<{ files: string[]; introspection: SchemaIntrospection }> {
    const core = await loadSemanticCore();
    const introspection = JSON.parse(core.introspectSchema(source)) as SchemaIntrospection;
    if (!introspection.ok) {
        throw new Error(introspection.error ?? "schema introspection failed");
    }
    const result = await emitTypescriptClient({
        outDir: resolve(outDir),
        introspection,
    });
    return { files: result.files, introspection };
}
