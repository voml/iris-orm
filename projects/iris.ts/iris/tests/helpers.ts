import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

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

export function wasmPlatformPackageInstalled(): boolean {
    try {
        readFileSync(join(pkgRoot, "../iris-unknown-wasm32/lib/iris.unknown-wasm32.wasm"));
        return true;
    } catch {
        return false;
    }
}
