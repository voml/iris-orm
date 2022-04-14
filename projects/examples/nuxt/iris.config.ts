import { defineConfig } from "@yydb/iris/types";

/** Nuxt Nitro — file-backed YYDB survives dev HMR (`server/utils/db.ts` singleton). */
export default defineConfig({
    schema: "schemas/blog.iris",
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
