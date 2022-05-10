import { createCli, normalizeArgv } from "@vmz/commander";

import { packageVersion } from "../node/versions.ts";
import { registerCapabilitiesCommand } from "./capabilities-cmd.ts";
import { registerCheckCommand } from "./check-cmd.ts";
import { IRIS_CLI_CATALOG } from "./catalog.ts";
import { registerDoctorCommand } from "./doctor-cmd.ts";
import { registerGenerateCommand } from "./generate-cmd.ts";
import { registerPushCommand } from "./push-cmd.ts";

function buildCli() {
    const cli = createCli("iris").catalog(() => IRIS_CLI_CATALOG).intro("iris.intro");

    registerCheckCommand(cli);
    registerGenerateCommand(cli);
    registerPushCommand(cli);
    registerDoctorCommand(cli);
    registerCapabilitiesCommand(cli);

    return cli;
}

export async function runCli(argv: string[] = process.argv): Promise<number> {
    const args = normalizeArgv(argv);
    if (args.includes("--version") || args.includes("-V")) {
        console.log(packageVersion);
        return 0;
    }

    try {
        return await buildCli().parse(argv);
    } catch (error) {
        console.error(`error: ${error instanceof Error ? error.message : String(error)}`);
        return 1;
    }
}
