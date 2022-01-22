/**
 * Iris CLI builder (`cac`) — no side effects on import.
 */
import cac from "cac";

import { version } from "../types/version.ts";

function notImplemented(name: string): void {
    console.error(`iris ${name}: not implemented yet in @yydb/iris/node (TypeScript host skeleton)`);
    process.exitCode = 1;
}

/** Build the `iris` CLI (exported for tests / embedding). */
export function createIrisCli() {
    const cli = cac("iris");

    cli.version(version);
    cli.help();

    cli.command("check [schema]", "Parse and validate a .iris schema")
        .option("--config <path>", "Path to iris.von")
        .action((_schema?: string) => {
            notImplemented("check");
        });

    cli.command("generate [schema]", "Generate host bindings via Dejavu (TS target)")
        .option("--config <path>", "Path to iris.von")
        .option("--out <dir>", "Output directory")
        .option("--target <name>", "Emitter target", {
            default: "typescript",
        })
        .action((_schema?: string) => {
            notImplemented("generate");
        });

    cli.command("doctor", "Local diagnostics (config / environment)")
        .option("--config <path>", "Path to iris.von")
        .action(() => {
            notImplemented("doctor");
        });

    cli.command("capabilities", "Print datasource capability summaries")
        .option("--config <path>", "Path to iris.von")
        .action(() => {
            notImplemented("capabilities");
        });

    return cli;
}
