import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import type { CheckSourceResult } from "../src/types/check-source.ts";

const pkgRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const winNode = join(pkgRoot, "../iris-win32-x64/iris.win32-x64-msvc.node");

export function srcImport(relativeFromPkgRoot: string): string {
    return new URL(relativeFromPkgRoot, new URL("../", import.meta.url)).href;
}

export function ensureNativeOverride(): void {
    if (!process.env.NAPI_RS_NATIVE_LIBRARY_PATH) {
        try {
            readFileSync(winNode);
            process.env.NAPI_RS_NATIVE_LIBRARY_PATH = winNode;
        } catch {
            // optional workspace artifact only
        }
    }
}

export function wasmArtifactBuilt(): boolean {
    try {
        readFileSync(join(pkgRoot, "../iris-unknown-wasm32/iris.unknown-wasm32.wasm"));
        return true;
    } catch {
        return false;
    }
}

export function summarizeCheck(result: CheckSourceResult) {
    return {
        ok: result.ok,
        tableCount: result.tableCount,
        schemaFingerprint: result.schemaFingerprint,
        generatorVersion: result.generatorVersion,
        error: result.error ?? null,
    };
}

export async function loadWasmBinding() {
    try {
        const mod = await import("@yydb/iris-unknown-wasm32");
        await mod.initWasm();
        return mod;
    } catch {
        const entry = new URL("../../iris-unknown-wasm32/src/index.ts", import.meta.url).href;
        const mod = await import(entry);
        await mod.initWasm();
        return mod;
    }
}
