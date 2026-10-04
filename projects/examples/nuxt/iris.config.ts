import { defineConfig } from "@yydb/iris";

/** Nuxt Nitro - file-backed YYDB survives dev HMR (`openDatabase` from `@iris/node.ts`). */
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
        out: "server/generated/iris",
        target: "typescript",
    },
});
