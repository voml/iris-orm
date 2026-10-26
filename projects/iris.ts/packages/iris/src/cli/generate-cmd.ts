import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

import type { Cli, ParsedOptions } from "@vmz/commander";

import { DEFAULT_TYPESCRIPT_GENERATE_OUT, resolveGenerateRoot } from "../node/generate-path.ts";
import { loadIrisNative } from "../node/load.ts";
import { loadProject, readProjectSchema } from "../node/project.ts";
import { markAction, str } from "./options.ts";

export function registerGenerateCommand(cli: Cli): void {
    cli.command("generate", "iris.cmd.generate")
        .option("--config <path>", "iris.opt.config")
        .option("--out <dir>", "iris.opt.out")
        .option("--target <name>", "iris.opt.target")
        .option("--contract <path>", "iris.opt.contract")
        .option("--operations <path>", "iris.opt.operations")
        .action(async (options) => cmdGenerate(options));
}

export async function cmdGenerate(options: ParsedOptions): Promise<number> {
    markAction();
    const core = loadIrisNative();
    const contractArg = str(options, "contract");
    const operationsArg = str(options, "operations");
    const useArtifacts = Boolean(contractArg || operationsArg);

    let target: string;
    let outRoot: string;
    let result;

    if (useArtifacts) {
        if (!contractArg || !operationsArg) {
            console.error("error: --contract and --operations must be used together (Route B)");
            return 1;
        }
        const contractJson = await readFile(resolve(contractArg), "utf8");
        const operationsJson = await readFile(resolve(operationsArg), "utf8");
        target = str(options, "target") ?? "typescript";
        outRoot = str(options, "out")
            ? resolve(str(options, "out")!)
            : resolve(DEFAULT_TYPESCRIPT_GENERATE_OUT);
        result = core.generateFromArtifacts(contractJson, operationsJson, target, outRoot);
    } else {
        const schemaArg = options._[0];
        const project = schemaArg ? null : await loadProject(str(options, "config") ?? process.cwd());
        const source = schemaArg ? await readFile(resolve(schemaArg), "utf8") : await readProjectSchema(project!);
        target = str(options, "target") ?? project?.generateTarget ?? "typescript";
        outRoot = str(options, "out")
            ? resolve(str(options, "out")!)
            : project
              ? resolveGenerateRoot(project.root, project.generateOut)
              : resolve(DEFAULT_TYPESCRIPT_GENERATE_OUT);
        result = core.generate(source, target, outRoot);
    }

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
}
