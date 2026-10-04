import type { Cli, ParsedOptions } from "@vmz/commander";

import { negotiateCapabilities } from "../runtime/negotiate-capabilities.ts";
import type { IrisBindingProfile, IrisHost } from "../types/profile.ts";
import { str } from "./options.ts";

export function registerCapabilitiesCommand(cli: Cli): void {
    cli.command("capabilities", "iris.cmd.capabilities")
        .option("--host <host>", "iris.opt.capabilities.host")
        .option("--profile <profile>", "iris.opt.capabilities.profile")
        .action((options) => cmdCapabilities(options));
}

function parseHost(value?: string): IrisHost {
    const host = (value ?? "node").toLowerCase();
    if (host === "web") {
        return "browser";
    }
    if (host === "node" || host === "browser" || host === "cloudflare-worker") {
        return host;
    }
    throw new Error(`unknown host \`${host}\` (use node|browser|cloudflare-worker)`);
}

function parseProfile(value?: string): IrisBindingProfile {
    const profile = (value ?? "memory").toLowerCase();
    if (profile === "memory" || profile === "sqlite" || profile === "project") {
        return profile;
    }
    throw new Error(`unknown profile \`${profile}\` (use memory|sqlite|project)`);
}

export function cmdCapabilities(options: ParsedOptions): number {
    const matrix = negotiateCapabilities({
        host: parseHost(str(options, "host")),
        profile: parseProfile(str(options, "profile")),
        bindingReady: false,
    });
    console.log(JSON.stringify(matrix, null, 2));
    return 0;
}
