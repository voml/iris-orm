/**
 * Iris CLI (`cac`) — Node-only; bin entry is `src/node/cli.ts`.
 */
import cac from "cac";

import { version } from "../types/version.ts";
import { checkSchemaFile } from "./check.ts";
import { printDoctorReport } from "./doctor.ts";

function notImplemented(name: string): void {
    console.error(`iris ${name}: not implemented in @yydb/iris/node yet — use Rust iris-tools for full semantics`);
    process.exitCode = 1;
}

/** Build the `iris` CLI (exported for tests / embedding). */
export function createIrisCli() {
    const cli = cac("iris");

    cli.version(version);
    cli.help();

    cli.command("check [schema]", "Parse and validate a .iris schema")
        .option("--config <path>", "Path to iris.von (ignored until project-aware check)")
        .action(async (schema?: string) => {
            if (!schema) {
                console.error("iris check: schema path required");
                process.exitCode = 1;
                return;
            }
            process.exitCode = await checkSchemaFile(schema);
        });

    cli.command("generate [schema]", "Generate host bindings via Dejavu")
        .option("--config <path>", "Path to iris.von")
        .option("--out <dir>", "Output directory")
        .option("--target <name>", "Emitter target", { default: "typescript" })
        .action((_schema?: string) => {
            notImplemented("generate");
        });

    cli.command("doctor", "Local diagnostics (config / environment)")
        .option("--config <path>", "Path to iris.von")
        .action(async () => {
            await printDoctorReport();
        });

    cli.command("capabilities", "Print datasource capability summaries")
        .option("--config <path>", "Path to iris.von")
        .action(() => {
            notImplemented("capabilities");
        });

    return cli;
}
