import { version } from "../types/version.ts";
import { IrisFacadeError } from "../types/errors.ts";
import { isOptionalPackageInstalled } from "./package-probe.ts";
import { loadNativeBinding, resolvePlatformPackageName } from "./native.ts";

const WASM_PACKAGE = "@yydb/iris-unknown-wasm32";

/** Print Node host / optional binding diagnostics (stdout). */
export async function printDoctorReport(): Promise<void> {
    const platformPkg = resolvePlatformPackageName();
    const lines = [
        `iris ${version} (@yydb/iris/node)`,
        `node ${process.version} · ${process.platform}-${process.arch}`,
        "",
        "Entry points:",
        "  @yydb/iris          browser / Worker (WASM inside)",
        "  @yydb/iris/node     Node N-API + iris CLI",
        "  @yydb/iris/types    protocol types only (no loader)",
        "",
        "Node N-API platform package:",
        platformPkg
            ? `  ${platformPkg}: ${isOptionalPackageInstalled(platformPkg) ? "installed" : "missing (optional)"}`
            : `  (no published package for ${process.platform}-${process.arch})`,
        "",
        "Browser WASM platform package:",
        `  ${WASM_PACKAGE}: ${isOptionalPackageInstalled(WASM_PACKAGE) ? "installed" : "missing (optional)"}`,
    ];

    try {
        const { packageName, module } = await loadNativeBinding();
        lines.push("", "N-API binding:", `  loaded: ${packageName}`, `  iris_version: ${module.irisVersion()}`);
    } catch (error) {
        const note = error instanceof IrisFacadeError ? error.message : error instanceof Error ? error.message : String(error);
        lines.push("", "N-API binding:", `  not loaded: ${note}`);
    }

    lines.push(
        "",
        "Browser WASM core: not wired (skeleton)",
        "",
        "Note: full generate/migrate runs on Rust iris-tools until N-API commands land.",
    );

    console.log(lines.join("\n"));
}
