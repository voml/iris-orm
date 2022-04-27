import type { CreateIrisDbBindingOptions, IrisDbBinding } from "../types/executor.ts";
import { createIrisDbBindingFromSession } from "../runtime/db-binding.ts";
import { openBindingSession } from "../runtime/open-binding-session.ts";
import { loadIrisNative } from "./load.ts";
import { loadProject, readProjectSchema } from "./project.ts";

/** Create internal binding support for generated `db` (not an application entry). */
export async function createIrisDbBinding(options: CreateIrisDbBindingOptions = {}): Promise<IrisDbBinding> {
    const core = loadIrisNative();
    const session = await openBindingSession("node", core, options, {
        openProjectSession: async (configPath, source) => {
            const project = await loadProject(configPath);
            const binding = core.openProjectSession!(project.runtimeConfig, source ?? "default");
            const schema = await readProjectSchema(project);
            return { session: binding, schema };
        },
    });
    return createIrisDbBindingFromSession(session);
}

/** @deprecated Use `createIrisDbBinding`. */
export const createIrisExecutor = createIrisDbBinding;

/** @deprecated Binding bring-up host; use generated `db`. */
export async function createIrisBindingHost(options: CreateIrisDbBindingOptions = {}) {
    const { buildRuntime } = await import("../runtime/build-runtime.ts");
    const core = loadIrisNative();
    return buildRuntime("node", core);
}
