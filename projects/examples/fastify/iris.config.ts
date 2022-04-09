import { defineConfig } from "@yydb/iris/types";

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
