import { readFile } from "node:fs/promises";

import { IrisFacadeError } from "../types/errors.ts";
import { loadNativeBinding } from "./native.ts";

/** Validate a `.iris` / VOS schema file on disk via Rust N-API. */
export async function checkSchemaFile(schemaPath: string): Promise<number> {
    const source = await readFile(schemaPath, "utf8");
    try {
        const { module } = await loadNativeBinding();
        const result = module.checkSource(source);
        if (result.ok) {
            console.log(`iris check: ok (${schemaPath}) — ${result.tableCount} table(s), fingerprint=${result.schemaFingerprint}`);
            return 0;
        }
        console.error(`error: ${schemaPath}: ${result.error ?? "schema validation failed"}`);
        return 1;
    } catch (error) {
        if (error instanceof IrisFacadeError && (error.code === "native-package-missing" || error.code === "native-unsupported-platform")) {
            console.error(`error: ${error.message}`);
            console.error("hint: install the matching optional @yydb/iris-* platform package (same version as @yydb/iris)");
            return 1;
        }
        throw error;
    }
}
