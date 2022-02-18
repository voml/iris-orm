import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);

/** npm semver for `@yydb/iris` (from package.json; not the Iris semantic core). */
export const packageVersion: string = require("../../package.json").version as string;

/** Iris semantic core version from Rust (`iris::version()` via N-API). */
export async function irisCoreVersion(): Promise<string> {
    const { loadSemanticCore } = await import("./native.ts");
    return (await loadSemanticCore()).irisVersion();
}
