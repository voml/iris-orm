import type { IrisSession, OpenSessionOptions } from "../types/session.ts";
import { buildRuntime } from "../runtime/build-runtime.ts";
import { loadSemanticCore } from "./native.ts";

/** Open a Node session on a foreign adapter (SQLite / Postgres / MySQL / project datasource). */
export async function openDatasourceSession(options: OpenSessionOptions = {}): Promise<IrisSession> {
    const core = await loadSemanticCore();
    return buildRuntime("node", core).openSession(options);
}
