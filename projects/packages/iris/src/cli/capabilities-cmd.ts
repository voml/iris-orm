import type { Cli } from "@vmz/commander";

export function registerCapabilitiesCommand(cli: Cli): void {
    cli.command("capabilities", "iris.cmd.capabilities").action(() => cmdCapabilities());
}

export function cmdCapabilities(): number {
    console.error("iris capabilities: not implemented in @yydb/iris yet");
    return 1;
}
