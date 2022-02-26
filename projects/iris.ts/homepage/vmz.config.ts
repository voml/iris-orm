import { defineConfig } from "@vmz/vmz";

export default defineConfig({
    // Documents use `@vmz/plugin-markdown-it` at build time (document integrate);
    // do not register the client MarkdownIt component — static-cdn SSR cannot
    // strip types from node_modules/runtime.ts (0.1.9).
    delivery: {
        default: "static",
        profiles: {
            static: { host: "browser", assembly: "static-cdn" },
        },
    },
});
