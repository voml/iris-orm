import { join } from "node:path";

/** Legacy `generate.out: "."` maps here; prefer an explicit path in `iris.config.ts`. */
export const DEFAULT_TYPESCRIPT_GENERATE_OUT = "src/generated/iris";

/** Resolve `generate.out` (project-relative) to the TypeScript client root. */
export function resolveGenerateRoot(projectRoot: string, generateOut: string): string {
    const relative = generateOut === "." ? DEFAULT_TYPESCRIPT_GENERATE_OUT : generateOut;
    return join(projectRoot, relative);
}
