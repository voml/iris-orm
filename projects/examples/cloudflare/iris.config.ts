import { defineConfig } from "@yydb/iris/types";

/**
 * Cloudflare Workers spike — schema + generate match other HTTP examples.
 *
 * Runtime: Workers cannot load `@yydb/iris/node` (N-API). Use `@iris/browser.ts` (WASM memory) in the Worker.
 * Persistence target: D1 (`wrangler.toml` binding `DB`) via SQL migrations until an Iris D1 session ships.
 *
 * Local `iris push` can target `.local/blog.sqlite` (same shape as D1 migrations).
 */
export default defineConfig({
    schema: "schemas/**/*.iris",
    datasources: {
        default: {
            kind: "sqlite",
            mode: "managed_push",
            path: ".local/blog.sqlite",
        },
    },
    generate: {
        out: "src/generated/iris",
        target: "typescript",
    },
});
