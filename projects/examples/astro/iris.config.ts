import { defineConfig } from "@yydb/iris/types";

/** Astro SSR (`@astrojs/node`) — file-backed YYDB survives dev HMR (`openDatabase` from `@iris/node.ts`). */
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
        out: "src/generated/iris",
        target: "typescript",
    },
});
