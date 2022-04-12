// @ts-check
import node from "@astrojs/node";
import { fileURLToPath } from "node:url";
import { defineConfig } from "astro/config";

const irisNativePackages = [
    "@yydb/iris",
    "@yydb/iris-win32-x64",
    "@yydb/iris-linux-x64",
    "@yydb/iris-linux-arm64",
    "@yydb/iris-darwin-x64",
    "@yydb/iris-darwin-arm64",
];

export default defineConfig({
    output: "server",
    adapter: node({ mode: "standalone" }),
    vite: {
        resolve: {
            alias: {
                "@iris": fileURLToPath(new URL("./src/generated/iris", import.meta.url)),
            },
        },
        ssr: {
            external: irisNativePackages,
        },
    },
});
