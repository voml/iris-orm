import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";

import { irisNativePackages } from "./svelte.config.js";

export default defineConfig({
    plugins: [sveltekit()],
    ssr: {
        external: irisNativePackages,
    },
});
