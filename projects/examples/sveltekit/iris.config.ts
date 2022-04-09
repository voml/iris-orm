import { defineIrisConfig } from "@yydb/iris/types";

export default defineIrisConfig({
    schema: "schemas/blog.iris",
    datasources: {
        default: {
            kind: "sqlite",
            mode: "managed_push",
            path: ":memory:",
        },
    },
    generate: {
        out: "generated/iris",
        target: "typescript",
    },
});
