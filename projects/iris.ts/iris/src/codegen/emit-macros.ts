import type { SchemaMacroModel } from "../types/schema-introspection.ts";

function macroReturnTs(returnType: string): string {
    switch (returnType.replace("?", "")) {
        case "utf8":
        case "uuid":
        case "decimal":
        case "datetime":
            return "string";
        case "bool":
            return "boolean";
        case "unit":
            return "void";
        default:
            // Named class/table returns stay as identifier; may be incomplete until MacroModel carries full types.
            return returnType.includes("::") || /^[A-Z]/.test(returnType) ? returnType : "unknown";
    }
}

/** Emit generated macro bindings (schema truth; names stay snake_case). */
export function emitMacros(macros: SchemaMacroModel[]): string {
    if (macros.length === 0) {
        return `import type { IrisDbBinding } from "@yydb/iris/types";

/** Schema macros (empty for this project). */
export class GeneratedMacros {
    constructor(private readonly binding: IrisDbBinding) {}
}
`;
    }

    const methods = macros
        .map((macro) => {
            const ret = macroReturnTs(macro.returnType);
            const retExpr = ret === "void" ? "await this.binding.execute(source, parameters)" : `(await this.binding.query(source, parameters)) as ${ret}`;
            return `
    async ${macro.name}(...args: unknown[]): Promise<${ret}> {
        const { source, parameters } = synthesizeMacroCall("${macro.name}", args);
        return ${retExpr};
    }`;
        })
        .join("\n");

    return `import type { IrisDbBinding } from "@yydb/iris/types";
import { synthesizeMacroCall } from "./synthesize.js";

/** Generated macro bindings from schema truth (not backend capability intersection). */
export class GeneratedMacros {
    constructor(private readonly binding: IrisDbBinding) {}
${methods}
}
`;
}
