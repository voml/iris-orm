import { createCli, normalizeArgv } from "@vmz/commander";

import { packageVersion } from "../node/versions.ts";
import { IrisConfigError } from "../types/errors.ts";
import { registerCapabilitiesCommand } from "./capabilities-cmd.ts";
import { registerCheckCommand } from "./check-cmd.ts";
import { IRIS_CLI_CATALOG } from "./catalog.ts";
import { registerDoctorCommand } from "./doctor-cmd.ts";
import { registerGenerateCommand } from "./generate-cmd.ts";
import { actionCountSnapshot } from "./options.ts";
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

function reportError(error: unknown): number {
    if (error instanceof IrisConfigError) {
        console.error(`error [${error.code}] ${error.configPath}: ${error.args.reason}`);
        if (error.cause !== undefined) {
            const cause = error.cause instanceof Error ? error.cause.message : String(error.cause);
            console.error(`  cause: ${cause}`);
        }
        return 1;
    }
    console.error(`error: ${error instanceof Error ? error.message : String(error)}`);
    return 1;
}

export async function runCli(argv: string[] = process.argv): Promise<number> {
    const args = normalizeArgv(argv);
    if (args.includes("--version") || args.includes("-V")) {
        console.log(packageVersion);
        return 0;
    }

    const actionsBefore = actionCountSnapshot();
    try {
        const code = await buildCli().parse(argv);
        return code === 1 && actionCountSnapshot() === actionsBefore ? 2 : code;
    } catch (error) {
        return reportError(error);
    }
}
