import { access } from "node:fs/promises";
import { resolve } from "node:path";

import { IrisFacadeError } from "../types/errors.ts";

const PROJECT_FILE = "iris.von";

/** Load an on-disk Iris project directory (Node-only). */
export async function loadProject(projectPath: string): Promise<{ readonly root: string; readonly config: string }> {
    const root = resolve(projectPath);
    const config = resolve(root, PROJECT_FILE);
    try {
        await access(config);
    } catch {
        throw new IrisFacadeError("project-missing", `@yydb/iris/node: ${PROJECT_FILE} not found under ${root}`);
    }
    throw new IrisFacadeError("project-not-implemented", `@yydb/iris/node: project loader not implemented yet (${config})`);
}
