import { readFile } from "node:fs/promises";

import { loadSemanticCore } from "./native.ts";
import { IrisFacadeError } from "../types/errors.ts";

/** Validate a `.iris` / VOS schema file on disk via Rust N-API. */
export async function checkSchemaFile(schemaPath: string): Promise<number> {
    const source = await readFile(schemaPath, "utf8");
    try {
        const core = await loadSemanticCore();
        const result = core.checkSource(source);
        if (result.ok) {
            console.log(`iris check: ok (${schemaPath}) — ${result.tableCount} table(s), fingerprint=${result.schemaFingerprint}`);
            return 0;
        }
        console.error(`error: ${schemaPath}: ${result.error ?? "schema validation failed"}`);
        return 1;
    } catch (error) {
        if (error instanceof IrisFacadeError && (error.code === "native-package-missing" || error.code === "native-unsupported-platform")) {
            console.error(`error: ${error.message}`);
            console.error("hint: reinstall @yydb/iris so npm can install the optional semantic core for your host");
            return 1;
        }
        throw error;
    }
}
