import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

import type { Cli, ParsedOptions } from "@vmz/commander";

import { DEFAULT_TYPESCRIPT_GENERATE_OUT, resolveGenerateRoot } from "../node/generate-path.ts";
import { loadIrisNative } from "../node/load.ts";
import { loadProject, readProjectSchema } from "../node/project.ts";
import { str } from "./options.ts";

export function registerGenerateCommand(cli: Cli): void {
    cli.command("generate", "iris.cmd.generate")
        .option("--config <path>", "iris.opt.config")
        .option("--out <dir>", "iris.opt.out")
        .option("--target <name>", "iris.opt.target")
        .action(async (options) => cmdGenerate(options));
}

export async function cmdGenerate(options: ParsedOptions): Promise<number> {
    try {
        const core = loadIrisNative();
        const schemaArg = options._[0];
        const project = schemaArg ? null : await loadProject(str(options, "config") ?? process.cwd());
        const source = schemaArg ? await readFile(resolve(schemaArg), "utf8") : await readProjectSchema(project!);
        const target = str(options, "target") ?? project?.generateTarget ?? "typescript";
        const outRoot = str(options, "out")
            ? resolve(str(options, "out")!)
            : project
              ? resolveGenerateRoot(project.root, project.generateOut)
              : resolve(DEFAULT_TYPESCRIPT_GENERATE_OUT);
        const result = core.generate(source, target, outRoot);
        if (!result.ok) {
            console.error(`error: ${result.error ?? "generate failed"}`);
            return 1;
        }
        console.log(`generated ${target} client (${result.files.length} files, fingerprint=${result.schemaFingerprint})`);
        console.log(`  output: ${result.outputPath}`);
        for (const file of result.files) {
            console.log(`  - ${file}`);
        }
        return 0;
    } catch (error) {
        console.error(`error: ${error instanceof Error ? error.message : String(error)}`);
        return 1;
    }
}
