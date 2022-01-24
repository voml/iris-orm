import { readFile } from "node:fs/promises";

import { IrisFacadeError } from "../types/errors.ts";
import { checkSource } from "../types/index.ts";
import { loadNativeBinding } from "./native.ts";

async function checkWithNative(source: string, schemaPath: string): Promise<number> {
    const { module } = await loadNativeBinding();
    const result = module.checkSource(source);
    if (result.ok) {
        console.log(
            `iris check: ok (${schemaPath}) — ${result.tableCount} table(s), fingerprint=${result.schemaFingerprint}`,
        );
        return 0;
    }
    console.error(`error: ${schemaPath}: ${result.error ?? "schema validation failed"}`);
    return 1;
}

function checkWithVosParser(source: string, schemaPath: string): number {
    const result = checkSource(source);
    if (result.ok) {
        console.log(`iris check: ok (${schemaPath}) [vos-parser fallback]`);
        return 0;
    }
    for (const d of result.diagnostics) {
        const where = d.line != null ? `${schemaPath}:${d.line}` : schemaPath;
        console.error(`error: ${where}: ${d.message}`);
    }
    return 1;
}

/** Validate a `.iris` / VOS schema file on disk. Prefers Rust N-API when available. */
export async function checkSchemaFile(schemaPath: string): Promise<number> {
    const source = await readFile(schemaPath, "utf8");
    try {
        return await checkWithNative(source, schemaPath);
    } catch (error) {
        if (
            error instanceof IrisFacadeError &&
            (error.code === "native-package-missing" || error.code === "native-unsupported-platform")
        ) {
            return checkWithVosParser(source, schemaPath);
        }
        throw error;
    }
}
