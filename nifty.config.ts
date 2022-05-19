// Nifty project configuration for iris-orm (hybrid cargo + pnpm).
import { defineConfig } from "@doki-land/nifty";

export default defineConfig({
    format: {
        preset: "nifty",
        style: {
            indentStyle: "space",
            indentWidth: 4,
            lineWidth: 144,
            quoteStyle: "double",
        },
        includes: [
            "package.json",
            "pnpm-workspace.yaml",
            "nifty.config.ts",
            "scripts/**",
            "projects/packages/**",
        ],
        excludes: [
            "**/node_modules/**",
            "**/dist/**",
            "**/target/**",
            "**/pnpm-lock.yaml",
            "**/*.md",
        ],
    },
    publish: {
        packages: [
            "@yydb/iris",
            "@yydb/iris-skills",
            "@yydb/iris-unknown-wasm32",
            "@yydb/iris-win32-x64",
            "@yydb/iris-linux-x64",
            "@yydb/iris-linux-arm64",
            "@yydb/iris-darwin-x64",
            "@yydb/iris-darwin-arm64",
        ],
    },
});
