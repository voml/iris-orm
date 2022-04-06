import { existsSync, statSync } from "node:fs";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { createJiti } from "jiti";

/**
 * @param {import("../types/config.ts").IrisUserConfig} config
 * @returns {import("../types/config.ts").IrisProjectDocument}
 */
function toProjectDocument(config) {
    return {
        format: "iris.project",
        version: 1,
        schema: config.schema,
        datasources: { ...(config.datasources ?? {}) },
        generate: {
            out: config.generate?.out ?? "generated/iris",
            target: config.generate?.target ?? "typescript",
        },
    };
}

export const CONFIG_FILE_NAMES = [
    "iris.config.ts",
    "iris.config.mts",
    "iris.config.mjs",
    "iris.config.js",
];

export const LEGACY_CONFIG_FILE = "iris.von";

export const RUNTIME_PROJECT_FILE = ".iris/project.von";

/**
 * @param {string} full
 * @returns {Promise<import("../types/config.ts").IrisUserConfig>}
 */
async function importMaybeTs(full) {
    const ext = full.slice(full.lastIndexOf(".")).toLowerCase();
    if (ext === ".ts" || ext === ".mts") {
        const jiti = createJiti(import.meta.url, {
            interopDefault: true,
            moduleCache: false,
        });
        return jiti(full);
    }
    const mod = await import(pathToFileURL(full).href);
    return mod.default ?? mod;
}

/**
 * @param {string} projectRoot
 * @returns {string | null}
 */
export function findAuthoringConfig(projectRoot) {
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
export function resolveProjectRoot(input) {
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
 * @param {string} configPath
 * @returns {Promise<import("../types/config.ts").IrisUserConfig>}
 */
export async function loadAuthoringConfig(configPath) {
    const cfg = await importMaybeTs(configPath);
    if (!cfg?.schema?.trim()) {
        throw new Error(`iris.config.ts: \`schema\` must point at on-disk .iris data (${configPath})`);
    }
    return cfg;
}

/**
 * Resolve the native project config path for Rust (`iris.von` or materialized `.iris/project.von`).
 *
 * @param {string} input project root, `iris.config.ts`, or legacy `iris.von`
 * @param {{ materializeRuntimeProject(projectDir: string, documentJson: string): string }} core
 * @returns {Promise<{ root: string, authoringConfig: string | null, runtimeConfig: string, document: import("../types/config.ts").IrisProjectDocument | null }>}
 */
export async function resolveNativeProjectConfig(input, core) {
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

    throw new Error(
        `@yydb/iris/node: iris project config not found under ${root} (expected iris.config.ts or ${LEGACY_CONFIG_FILE})`,
    );
}
