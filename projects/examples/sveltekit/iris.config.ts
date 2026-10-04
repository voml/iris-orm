import { defineConfig } from "@yydb/iris";

/** SvelteKit adapter-node - file-backed YYDB survives dev HMR (`openDatabase` from `@iris/node.ts`). */
export default defineConfig({
    schema: "schemas/**/*.iris",
    datasources: {
        default: {
            kind: "yydb",
            mode: "native_pull",
            path: ".iris/dev.yydb",
        },
    },
    generate: {
        out: "src/generated/iris",
        target: "typescript",
    },
});
