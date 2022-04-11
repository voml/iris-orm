import { createRequire } from "node:module";

const WEB_CORE_PACKAGE = "@yydb/iris-unknown-wasm32";
const require = createRequire(import.meta.url);

/** Whether the optional browser semantic core package resolves (Node doctor / tooling only). */
export function isBrowserSemanticCoreInstalled(): boolean {
    try {
        require.resolve(WEB_CORE_PACKAGE);
        return true;
    } catch {
        return false;
    }
}
