import { defineConfig } from "@yydb/iris";

/** Standalone Node server - ephemeral in-memory YYDB per process (`openDatabase` from `@iris/node.ts`). */
export default defineConfig({
    schema: "schemas/**/*.iris",
    datasources: {
        default: {
            kind: "yydb",
            mode: "native_pull",
            path: ":memory:",
        },
    },
    generate: {
        out: "src/generated/iris",
        target: "typescript",
    },
});
