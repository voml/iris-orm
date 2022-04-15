// @ts-check
import node from "@astrojs/node";
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
        ssr: {
            external: irisNativePackages,
        },
    },
});
