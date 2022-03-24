import { defineConfig } from "@vmz/vmz";
import shiki from "@vmz/plugin-shiki";

export default defineConfig({
    plugins: [shiki()],
    engines: { code: "shiki" },
    delivery: {
        default: "static",
        profiles: {
            static: { host: "browser", assembly: "static-cdn" },
        },
    },
});
