import { fileURLToPath } from "node:url";

import adapter from "@sveltejs/adapter-node";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

const irisNativePackages = [
    "@yydb/iris",
    "@yydb/iris-win32-x64",
    "@yydb/iris-linux-x64",
    "@yydb/iris-linux-arm64",
    "@yydb/iris-darwin-x64",
    "@yydb/iris-darwin-arm64",
];

/** @type {import('@sveltejs/kit').Config} */
const config = {
    preprocess: vitePreprocess(),
    kit: {
        adapter: adapter(),
        vite: {
            resolve: {
                alias: {
                    "@iris": fileURLToPath(new URL("./src/generated/iris", import.meta.url)),
                },
            },
        },
    },
};

export default config;

export { irisNativePackages };
