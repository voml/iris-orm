import type { Cli } from "@vmz/commander";

import { printDoctorReport } from "../node/doctor.ts";
import { markAction } from "./options.ts";

export function registerDoctorCommand(cli: Cli): void {
    cli.command("doctor", "iris.cmd.doctor").action(async () => cmdDoctor());
}

export async function cmdDoctor(): Promise<number> {
    markAction();
    await printDoctorReport();
    return 0;
}
