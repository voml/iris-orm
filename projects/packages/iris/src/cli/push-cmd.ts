import type { Cli, ParsedOptions } from "@vmz/commander";

import { loadIrisNative } from "../node/load.ts";
import { resolveProjectConfigPath } from "../node/project.ts";
import { flag, str } from "./options.ts";

function notImplemented(name: string): number {
    console.error(`iris ${name}: not implemented in @yydb/iris yet`);
    return 1;
}

export function registerPushCommand(cli: Cli): void {
    cli.command("push", "iris.cmd.push")
        .option("--config <path>", "iris.opt.config")
        .option("--source <name>", "iris.opt.source")
        .option("--out <dir>", "iris.opt.out")
        .option("--plan", "iris.opt.plan")
        .action(async (options) => cmdPush(options));
}

export async function cmdPush(options: ParsedOptions): Promise<number> {
    try {
        const config = await resolveProjectConfigPath(str(options, "config") ?? process.cwd());
        const core = loadIrisNative();
        const source = str(options, "source") ?? "default";
        const out = str(options, "out") ?? null;
        if (flag(options, "plan")) {
            const result = core.migratePlanCmd(config, source, out);
            if (!result.ok) {
                console.error(`error: ${result.error ?? "push plan failed"}`);
                return 1;
            }
            console.log(`push plan written: ${result.planPath}`);
            return 0;
        }
        if (typeof core.migrateRunCmd !== "function") {
            return notImplemented("push apply");
        }
        const result = core.migrateRunCmd(config, source, out, false);
        if (!result.ok) {
            console.error(`error: ${result.error ?? "push failed"}`);
            return 1;
        }
        const created = result.createdTables ?? [];
        console.log(
            created.length
                ? `push ok — created: ${created.join(", ")}`
                : "push ok — verify passed (no new tables)",
        );
        return 0;
    } catch (error) {
        console.error(`error: ${error instanceof Error ? error.message : String(error)}`);
        return 1;
    }
}
