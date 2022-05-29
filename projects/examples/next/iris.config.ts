import { defineConfig } from "@yydb/iris";

/** Next.js App Router - file-backed YYDB survives dev HMR (`.iris/dev.yydb`). */
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
        out: "lib/generated/iris",
        target: "typescript",
    },
});
