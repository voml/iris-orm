import { isBrowserSemanticCoreInstalled, isNodeSemanticCoreInstalled, loadSemanticCore } from "./native.ts";
import { IrisFacadeError } from "../types/errors.ts";
import { version } from "../types/version.ts";

/** Print Node host / optional binding diagnostics (stdout). */
export async function printDoctorReport(): Promise<void> {
    const lines = [
        `iris ${version} (@yydb/iris/node)`,
        `node ${process.version} · ${process.platform}-${process.arch}`,
        "",
        "Entry points:",
        "  @yydb/iris          browser / Worker",
        "  @yydb/iris/node     Node + iris CLI",
        "  @yydb/iris/types    protocol types only",
        "",
        "Semantic cores (optional, installed with @yydb/iris):",
        `  Node host:     ${isNodeSemanticCoreInstalled() ? "installed" : "missing"}`,
        `  Browser host:  ${isBrowserSemanticCoreInstalled() ? "installed" : "missing"}`,
    ];

    try {
        const core = await loadSemanticCore();
        lines.push("", "Node semantic core:", "  ready", `  iris_version: ${core.irisVersion()}`);
    } catch (error) {
        const note = error instanceof IrisFacadeError ? error.message : error instanceof Error ? error.message : String(error);
        lines.push("", "Node semantic core:", `  not ready: ${note}`);
    }

    lines.push("", "Note: full generate/migrate runs on Rust iris-tools until N-API commands land.");

    console.log(lines.join("\n"));
}
