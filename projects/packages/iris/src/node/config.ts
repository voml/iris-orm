import { existsSync, statSync } from "node:fs";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { IrisConfigError } from "../types/errors.ts";
import { toProjectDocument } from "../types/config.ts";
import type { IrisProjectDocument, IrisUserConfig } from "../types/config.ts";

export const CONFIG_FILE_NAMES = [
    "iris.config.ts",
    "iris.config.mts",
    "iris.config.mjs",
    "iris.config.js",
];

export const LEGACY_CONFIG_FILE = "iris.von";

export const RUNTIME_PROJECT_FILE = ".iris/project.von";

/**
 * @param {string} projectRoot
 * @returns {string | null}
 */
export function findAuthoringConfig(projectRoot: string): string | null {
    for (const name of CONFIG_FILE_NAMES) {
        const full = join(projectRoot, name);
        if (existsSync(full)) {
            return full;
        }
    }
    return null;
}

/**
 * @param {string} input
 * @returns {string}
 */
export function resolveProjectRoot(input: string): string {
    const resolved = resolve(input);
    if (existsSync(resolved)) {
        const stat = statSync(resolved);
        if (stat.isDirectory()) {
            return resolved;
        }
        if (stat.isFile()) {
            return dirname(resolved);
        }
    }
    if (
        CONFIG_FILE_NAMES.some((name) => resolved.endsWith(name)) ||
        resolved.endsWith(LEGACY_CONFIG_FILE) ||
        resolved.endsWith(RUNTIME_PROJECT_FILE)
    ) {
        return dirname(resolved);
    }
    return resolved;
}

/**
 * Load an `iris.config.*` module with the host Node runtime (native type stripping).
 *
 * @param {string} configPath
 * @returns {Promise<IrisUserConfig>}
 */
export async function loadAuthoringConfig(configPath: string): Promise<IrisUserConfig> {
    let loaded: unknown;
    try {
        const mod = await import(pathToFileURL(configPath).href);
        loaded = mod.default ?? mod;
    } catch (error) {
        throw new IrisConfigError(
            "config-load-failed",
            configPath,
            { reason: "failed to execute project config" },
            error,
        );
    }

    const config = loaded as IrisUserConfig | undefined;
    if (!config?.schema?.trim()) {
        throw new IrisConfigError(
            "config-invalid",
            configPath,
            { reason: "`schema` must point at on-disk .iris data" },
        );
    }
    return config;
}

/**
 * Resolve the native project config path for Rust (`iris.von` or materialized `.iris/project.von`).
 *
 * @param {string} input project root, `iris.config.ts`, or legacy `iris.von`
 * @param {{ materializeRuntimeProject(projectDir: string, documentJson: string): string }} core
 * @returns {Promise<{ root: string, authoringConfig: string | null, runtimeConfig: string, document: IrisProjectDocument | null }>}
 */
export async function resolveNativeProjectConfig(input: string, core: { materializeRuntimeProject(projectDir: string, documentJson: string): string }): Promise<{
    root: string;
    authoringConfig: string | null;
    runtimeConfig: string;
    document: IrisProjectDocument | null;
}> {
    const root = resolveProjectRoot(input);
    const authoringConfig = findAuthoringConfig(root);
    if (authoringConfig) {
        const userConfig = await loadAuthoringConfig(authoringConfig);
        const document = toProjectDocument(userConfig);
        const runtimeConfig = core.materializeRuntimeProject(root, JSON.stringify(document));
        return { root, authoringConfig, runtimeConfig, document };
    }

    const legacy = isAbsolute(input) ? input : join(root, LEGACY_CONFIG_FILE);
    if (existsSync(legacy)) {
        return { root, authoringConfig: null, runtimeConfig: legacy, document: null };
    }
    if (existsSync(join(root, RUNTIME_PROJECT_FILE))) {
        const runtimeConfig = join(root, RUNTIME_PROJECT_FILE);
        return { root, authoringConfig: null, runtimeConfig, document: null };
    }
    if (input.endsWith(LEGACY_CONFIG_FILE) && existsSync(resolve(input))) {
        const runtimeConfig = resolve(input);
        return { root: dirname(runtimeConfig), authoringConfig: null, runtimeConfig, document: null };
    }

    throw new IrisConfigError(
        "config-not-found",
        root,
        { reason: `expected iris.config.ts or ${LEGACY_CONFIG_FILE}` },
    );
}
