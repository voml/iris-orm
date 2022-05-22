import type { Cli, ParsedOptions } from "@vmz/commander";

import { checkSchemaFile } from "../node/check.ts";
import { loadIrisNative } from "../node/load.ts";
import { loadProject, readProjectSchema } from "../node/project.ts";
import { str } from "./options.ts";

export function registerCheckCommand(cli: Cli): void {
    cli.command("check", "iris.cmd.check")
        .option("--config <path>", "iris.opt.config")
        .action(async (options) => cmdCheck(options));
}

export async function cmdCheck(options: ParsedOptions): Promise<number> {
    const schema = options._[0];
    if (schema) {
        return await checkSchemaFile(schema);
    }
    try {
        const project = await loadProject(str(options, "config") ?? process.cwd());
        const source = await readProjectSchema(project);
        const core = loadIrisNative();
        const result = core.checkSource(source);
        if (result.ok) {
            console.log(
                `iris check: ok (${project.schemaGlob}) — ${result.tableCount} table(s), fingerprint=${result.schemaFingerprint}`,
            );
            return 0;
        }
        console.error(`error: ${result.error ?? "schema validation failed"}`);
        return 1;
    } catch (error) {
        console.error(`error: ${error instanceof Error ? error.message : String(error)}`);
        return 1;
    }
}
