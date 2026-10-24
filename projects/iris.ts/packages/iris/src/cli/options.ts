import type { ParsedOptions } from "@vmz/commander";

let actionCount = 0;

/** Mark that a command action started (used to separate argv errors from action results). */
export function markAction(): void {
    actionCount += 1;
}

/** Number of command actions started in this process. */
export function actionCountSnapshot(): number {
    return actionCount;
}

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
