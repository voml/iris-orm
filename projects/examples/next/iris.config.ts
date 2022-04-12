import { defineConfig } from "@yydb/iris/types";

/** Next.js App Router — file sqlite survives dev HMR (`file:.iris/dev.sqlite`). */
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
