import { defineConfig } from "@yydb/iris/types";

/** Astro SSR (`@astrojs/node`) — file-backed YYDB survives dev HMR (`src/lib/server/db.ts`). */
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
        out: ".",
        target: "typescript",
    },
});
