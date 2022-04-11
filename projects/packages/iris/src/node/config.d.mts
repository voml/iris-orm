import type { IrisProjectDocument, IrisUserConfig } from "../types/config.ts";

export const CONFIG_FILE_NAMES: readonly string[];
export const LEGACY_CONFIG_FILE: string;
export const RUNTIME_PROJECT_FILE: string;

export function findAuthoringConfig(projectRoot: string): string | null;
export function resolveProjectRoot(input: string): string;
export function loadAuthoringConfig(configPath: string): Promise<IrisUserConfig>;

export function resolveNativeProjectConfig(
    input: string,
    core: { materializeRuntimeProject(projectDir: string, documentJson: string): string },
): Promise<{
    root: string;
    authoringConfig: string | null;
    runtimeConfig: string;
    document: IrisProjectDocument | null;
}>;
