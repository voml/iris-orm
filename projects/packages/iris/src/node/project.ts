import { resolve } from "node:path";

import { IrisFacadeError } from "../types/errors.ts";
import { loadSemanticCore } from "./native.ts";

const PROJECT_FILE = "iris.von";

/** Load an on-disk Iris project directory (Node-only). */
export async function loadProject(projectPath: string): Promise<{
    readonly root: string;
    readonly config: string;
    readonly schemaGlob: string;
    readonly generateOut: string;
    readonly generateTarget: string;
}> {
    const core = await loadSemanticCore();
    const root = resolve(projectPath);
    const config = root.endsWith(PROJECT_FILE) ? root : resolve(root, PROJECT_FILE);
    try {
        const loaded = core.loadProject(config);
        return {
            root: loaded.root,
            config: loaded.config,
            schemaGlob: loaded.schemaGlob,
            generateOut: loaded.generateOut,
            generateTarget: loaded.generateTarget,
        };
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        if (message.includes("not found") || message.includes("No such file")) {
            throw new IrisFacadeError("project-missing", `@yydb/iris/node: ${PROJECT_FILE} not found under ${root}`);
        }
        throw new IrisFacadeError("project-load-failed", `@yydb/iris/node: ${message}`);
    }
}

/** Read merged schema text for a loaded project (Node-only). */
export async function readProjectSchema(project: { root: string; schemaGlob: string }): Promise<string> {
    const core = await loadSemanticCore();
    return core.readSchema(project.root, project.schemaGlob);
}
