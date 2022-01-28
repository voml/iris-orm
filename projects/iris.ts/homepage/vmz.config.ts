import markdownIt from "@vmz/plugin-markdown-it";
import { defineConfig } from "@vmz/vmz";

export default defineConfig({
    plugins: [markdownIt],
    delivery: {
        default: "web-static",
        profiles: {
            "web-static": { host: "browser", assembly: "local-static" },
        },
    },
});
