import type { Cli } from "@vmz/commander";

import { printDoctorReport } from "../node/doctor.ts";

export function registerDoctorCommand(cli: Cli): void {
    cli.command("doctor", "iris.cmd.doctor").action(async () => cmdDoctor());
}

export async function cmdDoctor(): Promise<number> {
    await printDoctorReport();
    return 0;
}
