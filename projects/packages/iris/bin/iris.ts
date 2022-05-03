#!/usr/bin/env node
/**
 * `iris` CLI — Node-only entry (`package.json#bin.iris`).
 */
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

import cac from "cac";

import { checkSchemaFile } from "../src/node/check.ts";
import { printDoctorReport } from "../src/node/doctor.ts";
import { loadIrisNative } from "../src/node/load.ts";
import { DEFAULT_TYPESCRIPT_GENERATE_OUT, resolveGenerateRoot } from "../src/node/generate-path.ts";
import { loadProject, readProjectSchema, resolveProjectConfigPath } from "../src/node/project.ts";
import { packageVersion } from "../src/node/versions.ts";

function notImplemented(name: string): void {
    console.error(`iris ${name}: not implemented in @yydb/iris yet`);
    process.exitCode = 1;
}

const cli = cac("iris");

cli.version(packageVersion);
cli.help();

cli.command("check [schema]", "Validate schema + generated client drift")
    .option("--config <path>", "Project root or iris.config.ts")
    .action(async (schema?: string, options?: { config?: string }) => {
        if (schema) {
            process.exitCode = await checkSchemaFile(schema);
            return;
        }
        try {
            const project = await loadProject(options?.config ?? process.cwd());
            const source = await readProjectSchema(project);
            const core = loadIrisNative();
            const result = core.checkSource(source);
            if (result.ok) {
                console.log(
                    `iris check: ok (${project.schemaGlob}) — ${result.tableCount} table(s), fingerprint=${result.schemaFingerprint}`,
                );
                process.exitCode = 0;
                return;
            }
            console.error(`error: ${result.error ?? "schema validation failed"}`);
            process.exitCode = 1;
        } catch (error) {
            console.error(`error: ${error instanceof Error ? error.message : String(error)}`);
            process.exitCode = 1;
        }
    });

cli.command("generate [schema]", "Generate Iris client from .iris schema")
    .option("--config <path>", "Project root or iris.config.ts")
    .option("--out <dir>", "Generated client root (defaults to iris.config.ts generate.out)")
    .option("--target <name>", "Emitter target (defaults to iris.config.ts generate.target or typescript)")
    .action(async (schema?: string, options?: { out?: string; target?: string; config?: string }) => {
        try {
            const core = loadIrisNative();
            const project = schema ? null : await loadProject(options?.config ?? process.cwd());
            const source = schema ? await readFile(resolve(schema), "utf8") : await readProjectSchema(project!);
            const target = options?.target ?? project?.generateTarget ?? "typescript";
            const outRoot = options?.out
                ? resolve(options.out)
                : project
                  ? resolveGenerateRoot(project.root, project.generateOut)
                  : resolve(DEFAULT_TYPESCRIPT_GENERATE_OUT);
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
    .option("--config <path>", "Project root or iris.config.ts")
    .option("--source <name>", "Datasource name", { default: "default" })
    .option("--out <dir>", "Output directory for push plan artifacts")
    .option("--plan", "Plan only (no apply)")
    .action(async (options?: { config?: string; source?: string; out?: string; plan?: boolean }) => {
        try {
            const config = await resolveProjectConfigPath(options?.config ?? process.cwd());
            const core = loadIrisNative();
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
    .option("--config <path>", "Project root or iris.config.ts")
    .action(async () => {
        await printDoctorReport();
    });

cli.command("capabilities", "Print datasource capability summaries")
    .option("--config <path>", "Project root or iris.config.ts")
    .action(() => {
        notImplemented("capabilities");
    });

cli.parse();
