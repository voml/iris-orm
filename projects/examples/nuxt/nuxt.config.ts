import { fileURLToPath } from "node:url";

const irisNativePackages = [
    "@yydb/iris",
    "@yydb/iris-win32-x64",
    "@yydb/iris-linux-x64",
    "@yydb/iris-linux-arm64",
    "@yydb/iris-darwin-x64",
    "@yydb/iris-darwin-arm64",
];

export default defineNuxtConfig({
    alias: {
        "@iris": fileURLToPath(new URL("./server/generated/iris", import.meta.url)),
    },
    devtools: { enabled: false },
    compatibilityDate: "2025-07-15",
    nitro: {
        externals: {
            external: irisNativePackages,
        },
    },
    vite: {
        ssr: {
            external: irisNativePackages,
        },
    },
});
