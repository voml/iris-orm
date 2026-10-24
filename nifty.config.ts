// Nifty project configuration for iris-orm (hybrid cargo + pnpm).
// Trusted Publisher contract: repo voml/iris-orm, workflow publish-npm.yml, environment NPM_PUBLISH (see scripts/README.md).
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
            "projects/iris.ts/packages/**",
            "projects/iris.ts/platforms/**",
            "projects/iris.ts/homepage/**",
        ],
        excludes: [
            "**/node_modules/**",
            "**/dist/**",
            "**/target/**",
            "**/pnpm-lock.yaml",
            "**/*.md",
        ],
    },
    publish: {},
});
