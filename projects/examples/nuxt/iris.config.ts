import { defineConfig } from "@yydb/iris";

/** Nuxt Nitro - file-backed YYDB survives dev HMR (`Database.open` from `@iris/node.ts`). */
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
