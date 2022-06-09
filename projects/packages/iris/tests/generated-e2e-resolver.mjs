import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const irisPkgRoot = fileURLToPath(new URL("../", import.meta.url));

function irisSrc(subpath) {
    return pathToFileURL(join(irisPkgRoot, subpath));
}

const alias = new Map([
    ["@yydb/iris/types", irisSrc("src/types/index.ts")],
    ["@yydb/iris/node", irisSrc("src/node/index.ts")],
]);

export async function resolve(specifier, context, nextResolve) {
    const mapped = alias.get(specifier);
    if (mapped) {
        return { url: mapped.href, shortCircuit: true };
    }

    if (
        (specifier.startsWith("./") || specifier.startsWith("../")) &&
        specifier.endsWith(".js") &&
        context.parentURL?.includes("/generated/iris/")
    ) {
        const tsSpecifier = specifier.replace(/\.js$/, ".ts");
        return nextResolve(tsSpecifier, context);
    }

    return nextResolve(specifier, context);
}
