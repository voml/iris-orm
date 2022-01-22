import type { IrisRuntime } from "../types/protocol.ts";

export type CreateIrisNodeOptions = {
    /** Path to `iris.von` or project root. */
    project?: string;
};

/** Create a Node Iris runtime (N-API semantic core). */
export async function createIris(_options: CreateIrisNodeOptions = {}): Promise<IrisRuntime> {
    throw new Error("@yydb/iris/node: createIris is not implemented yet (N-API skeleton)");
}

/** Load an on-disk Iris project (Node-only). */
export async function loadProject(projectPath: string): Promise<unknown> {
    throw new Error(
        `@yydb/iris/node: loadProject is not implemented yet for ${projectPath}`,
    );
}
