import type { IrisTooling } from "../types/tooling.ts";
import { parseIntrospectionJson } from "../runtime/parse.ts";
import { loadIrisNative } from "./load.ts";

/** Tooling surface for CLI / agents (not application ORM). */
export async function createIrisTooling(): Promise<IrisTooling> {
    const core = loadIrisNative();
    return {
        checkSchema: (source) => core.checkSource(source),
        introspectSchema: (source) => parseIntrospectionJson(core.introspectSchema(source)),
    };
}
