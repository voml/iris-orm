import { defineConfig } from "@yydb/iris/types";

/** SvelteKit adapter-node — file sqlite survives dev HMR (`src/lib/server/db.ts`). */
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
