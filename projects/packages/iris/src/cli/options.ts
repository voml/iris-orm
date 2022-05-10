import type { ParsedOptions } from "@vmz/commander";

export function str(options: ParsedOptions, key: string): string | undefined {
    const value = options[key];
    return typeof value === "string" && value.length > 0 ? value : undefined;
}

export function flag(options: ParsedOptions, key: string): boolean {
    return options[key] === true;
}

export function firstPositional(options: ParsedOptions): string | undefined {
    return options._[0];
}
