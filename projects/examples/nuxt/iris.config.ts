import { defineConfig } from "@yydb/iris/types";

/** Nuxt Nitro — file sqlite survives dev HMR (`server/utils/db.ts` singleton). */
export default defineConfig({
    schema: "schemas/blog.iris",
    datasources: {
        default: {
            kind: "sqlite",
            mode: "managed_push",
            path: "file:.iris/dev.sqlite",
        },
    },
    generate: {
        out: ".",
        target: "typescript",
    },
});
