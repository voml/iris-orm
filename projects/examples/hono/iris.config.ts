import { defineConfig } from "@yydb/iris/types";

/** Standalone Node server — ephemeral `:memory:` per process (`src/db.ts` singleton). */
export default defineConfig({
    schema: "schemas/blog.iris",
    datasources: {
        default: {
            kind: "sqlite",
            mode: "managed_push",
            path: ":memory:",
        },
    },
    generate: {
        out: ".",
        target: "typescript",
    },
});
