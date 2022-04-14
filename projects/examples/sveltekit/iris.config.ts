import { defineConfig } from "@yydb/iris/types";

/** SvelteKit adapter-node — file-backed YYDB survives dev HMR (`src/lib/server/db.ts`). */
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
