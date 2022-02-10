/**
 * Iris CLI (`cac`) — Node-only; bin entry is `src/node/cli.ts`.
 */
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

import cac from "cac";

import { version } from "../types/version.ts";
import { checkSchemaFile } from "./check.ts";
import { printDoctorReport } from "./doctor.ts";
import { generateTypescriptClient } from "./generate-typescript.ts";
import { loadSemanticCore } from "./native.ts";
import { loadProject, readProjectSchema } from "./project.ts";

function notImplemented(name: string): void {
    console.error(`iris ${name}: not implemented in @yydb/iris/node yet — use Rust iris-tools for full semantics`);
    process.exitCode = 1;
}

/** Build the `iris` CLI (exported for tests / embedding). */
export function createIrisCli() {
    const cli = cac("iris");

    cli.version(version);
    cli.help();

    cli.command("check [schema]", "Validate schema + generated client drift")
        .option("--config <path>", "Path to iris.von")
        .action(async (schema?: string) => {
            if (!schema) {
                console.error("iris check: schema path required");
                process.exitCode = 1;
                return;
            }
            process.exitCode = await checkSchemaFile(schema);
        });

    cli.command("generate [schema]", "Generate TypeScript Iris client from .iris schema")
        .option("--config <path>", "Path to iris.von")
        .option("--out <dir>", "Project root for generated/iris output", { default: "." })
        .option("--target <name>", "Emitter target", { default: "typescript" })
        .action(async (schema?: string, options?: { out?: string; target?: string; config?: string }) => {
            const target = options?.target ?? "typescript";
            if (target === "rust") {
                try {
                    const core = await loadSemanticCore();
                    const source = schema
                        ? await readFile(resolve(schema), "utf8")
                        : await readProjectSchema(await loadProject(options?.config ?? process.cwd()));
                    const result = core.generateRust(source, resolve(options?.out ?? "generated"));
                    if (!result.ok) {
                        console.error(`error: ${result.error ?? "generate failed"}`);
                        process.exitCode = 1;
                        return;
                    }
                    console.log(`generated ${result.outputPath} (fingerprint=${result.schemaFingerprint})`);
                } catch (error) {
                    console.error(`error: ${error instanceof Error ? error.message : String(error)}`);
                    process.exitCode = 1;
                }
                return;
            }
            if (target !== "typescript") {
                console.error(`iris generate: unsupported target ${target} (use typescript or rust)`);
                process.exitCode = 1;
                return;
            }
            try {
                const source = schema
                    ? await readFile(resolve(schema), "utf8")
                    : await readProjectSchema(await loadProject(options?.config ?? process.cwd()));
                const result = await generateTypescriptClient(source, resolve(options?.out ?? "."));
                console.log(
                    `generated TypeScript client (${result.files.length} files, fingerprint=${result.introspection.schemaFingerprint})`,
                );
                for (const file of result.files) {
                    console.log(`  - ${file}`);
                }
            } catch (error) {
                console.error(`error: ${error instanceof Error ? error.message : String(error)}`);
                process.exitCode = 1;
            }
        });

    cli.command("push", "Push local schema to datasource (schema -> database)")
        .option("--config <path>", "Path to iris.von")
        .option("--source <name>", "Datasource name", { default: "default" })
        .option("--out <dir>", "Output directory for push plan artifacts")
        .option("--plan", "Plan only (no apply)")
        .action(async (options?: { config?: string; source?: string; out?: string; plan?: boolean }) => {
            const config = resolve(options?.config ?? "iris.von");
            try {
                const core = await loadSemanticCore();
                if (options?.plan) {
                    const result = core.migratePlanCmd(config, options?.source ?? "default", options?.out ?? null);
                    if (!result.ok) {
                        console.error(`error: ${result.error ?? "push plan failed"}`);
                        process.exitCode = 1;
                        return;
                    }
                    console.log(`push plan written: ${result.planPath}`);
                    return;
                }
                notImplemented("push apply");
            } catch (error) {
                console.error(`error: ${error instanceof Error ? error.message : String(error)}`);
                process.exitCode = 1;
            }
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
