import type { Cli, ParsedOptions } from "@vmz/commander";

import { negotiateCapabilities } from "../runtime/negotiate-capabilities.ts";
import type { IrisHost, IrisStorageProfile } from "../types/profile.ts";
import { markAction, str } from "./options.ts";

export function registerCapabilitiesCommand(cli: Cli): void {
    cli.command("capabilities", "iris.cmd.capabilities")
        .option("--host <host>", "iris.opt.capabilities.host")
        .option("--storage <storage>", "iris.opt.capabilities.storage")
        .action((options) => cmdCapabilities(options));
}

function parseHost(value?: string): IrisHost {
    const host = (value ?? "node").toLowerCase();
    if (host === "web") {
        return "browser";
    }
    if (host === "node" || host === "browser" || host === "cloudflare-worker" || host === "workerd" || host === "wasi") {
        return host;
    }
    throw new Error(`unknown host \`${host}\` (use node|browser|cloudflare-worker|workerd|wasi)`);
}

function parseStorage(value?: string): IrisStorageProfile {
    const storage = (value ?? "memory").toLowerCase();
    if (storage === "memory" || storage === "sqlite" || storage === "d1" || storage === "yyds" || storage === "remote-http") {
        return storage;
    }
    throw new Error(`unknown storage \`${storage}\` (use memory|sqlite|d1|yyds|remote-http)`);
}

export function cmdCapabilities(options: ParsedOptions): number {
    markAction();
    const matrix = negotiateCapabilities({
        host: parseHost(str(options, "host")),
        storage: parseStorage(str(options, "storage")),
        bindingReady: false,
    });
    console.log(JSON.stringify(matrix, null, 2));
    return 0;
}
