import { IrisFacadeError } from "../types/errors.ts";
import type { IrisProjectDocument, IrisUserConfig } from "../types/config.ts";
import { loadIrisNative } from "./load.ts";
import { findAuthoringConfig, loadAuthoringConfig, resolveNativeProjectConfig, resolveProjectRoot } from "./config.ts";

export type LoadedIrisProject = {
    readonly root: string;
    /** Authoring config path (`iris.config.ts`) when present. */
    readonly authoringConfig: string | null;
    /** Native runtime config path (`.iris/project.von` or legacy `iris.von`). */
    readonly runtimeConfig: string;
    readonly schemaGlob: string;
    readonly generateOut: string;
    readonly generateTarget: string;
    readonly document: IrisProjectDocument | null;
};

/** Load an on-disk Iris project (prefers `iris.config.ts`, falls back to legacy `iris.von`). */
export async function loadProject(projectPath: string): Promise<LoadedIrisProject> {
    const core = loadIrisNative();
    if (typeof core.materializeRuntimeProject !== "function") {
        throw new IrisFacadeError(
            "project-load-failed",
            "@yydb/iris/node: native core missing materializeRuntimeProject (rebuild N-API)",
        );
    }
    try {
        const resolved = await resolveNativeProjectConfig(projectPath, core);
        const loaded = core.loadProject(resolved.runtimeConfig);
        return {
            root: loaded.root,
            authoringConfig: resolved.authoringConfig,
            runtimeConfig: resolved.runtimeConfig,
            schemaGlob: loaded.schemaGlob,
            generateOut: loaded.generateOut,
            generateTarget: loaded.generateTarget,
            document: resolved.document,
        };
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        throw new IrisFacadeError("project-load-failed", `@yydb/iris/node: ${message}`);
    }
}

/** Load `iris.config.ts` from a project root (throws when missing). */
export async function loadIrisConfig(projectPath: string): Promise<IrisUserConfig> {
    const root = resolveProjectRoot(projectPath);
    const configPath = findAuthoringConfig(root);
    if (!configPath) {
        throw new IrisFacadeError("project-missing", `@yydb/iris/node: iris.config.ts not found under ${root}`);
    }
    return loadAuthoringConfig(configPath);
}

/** Read merged schema **data** for a loaded project (from paths declared in config, not from config itself). */
export async function readProjectSchema(project: { root: string; schemaGlob: string }): Promise<string> {
    const core = loadIrisNative();
    return core.readSchema(project.root, project.schemaGlob);
}

/** Resolve the native config path used by CLI / migrate (`iris.config.ts` → `.iris/project.von`). */
export async function resolveProjectConfigPath(projectPath: string): Promise<string> {
    const core = loadIrisNative();
    const resolved = await resolveNativeProjectConfig(projectPath, core);
    return resolved.runtimeConfig;
}
