import { defineConfig } from "@yydb/iris/types";

/** Standalone Node server — ephemeral in-memory YYDB per process (`src/db.ts` singleton). */
export default defineConfig({
    schema: "schemas/blog.iris",
    datasources: {
        default: {
            kind: "yydb",
            mode: "native_pull",
            path: ":memory:",
        },
    },
    generate: {
        out: ".",
        target: "typescript",
    },
});
