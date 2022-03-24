#!/usr/bin/env node
/**
 * `iris` CLI — Node-only entry (`package.json#bin.iris`).
 */
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

import cac from "cac";

import { checkSchemaFile } from "../src/node/check.ts";
import { printDoctorReport } from "../src/node/doctor.ts";
import { loadSemanticCore } from "../src/node/native.ts";
import { loadProject, readProjectSchema } from "../src/node/project.ts";
import { packageVersion } from "../src/node/versions.ts";

function notImplemented(name: string): void {
    console.error(`iris ${name}: not implemented in @yydb/iris/node yet — use Rust iris-tools for full semantics`);
    process.exitCode = 1;
}

const cli = cac("iris");

cli.version(packageVersion);
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

cli.command("generate [schema]", "Generate Iris client from .iris schema")
    .option("--config <path>", "Path to iris.von")
    .option("--out <dir>", "Output project root (writes generated/iris/<target>/ under this path)")
    .option("--target <name>", "Emitter target (defaults to iris.von generate.target or typescript)")
    .action(async (schema?: string, options?: { out?: string; target?: string; config?: string }) => {
        try {
            const core = await loadSemanticCore();
            const project = schema ? null : await loadProject(options?.config ?? process.cwd());
            const source = schema ? await readFile(resolve(schema), "utf8") : await readProjectSchema(project!);
            const target = options?.target ?? project?.generateTarget ?? "typescript";
            const outRoot = options?.out ? resolve(options.out) : project ? resolve(project.root, project.generateOut) : resolve(".");
            const result = core.generate(source, target, outRoot);
            if (!result.ok) {
                console.error(`error: ${result.error ?? "generate failed"}`);
                process.exitCode = 1;
                return;
            }
            console.log(`generated ${target} client (${result.files.length} files, fingerprint=${result.schemaFingerprint})`);
            console.log(`  output: ${result.outputPath}`);
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
            if (typeof core.migrateRunCmd !== "function") {
                notImplemented("push apply");
                return;
            }
            const result = core.migrateRunCmd(config, options?.source ?? "default", options?.out ?? null, false);
            if (!result.ok) {
                console.error(`error: ${result.error ?? "push failed"}`);
                process.exitCode = 1;
                return;
            }
            const created = result.createdTables ?? [];
            console.log(
                created.length
                    ? `push ok — created: ${created.join(", ")}`
                    : "push ok — verify passed (no new tables)",
            );
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

cli.parse();
