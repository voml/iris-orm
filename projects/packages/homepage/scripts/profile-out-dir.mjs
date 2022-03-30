#!/usr/bin/env node
/**
 * Homepage output layout: dist/<target>/
 *
 * - target `cdn` = static artifact for any CDN / object storage
 *   (Cloudflare Pages, Netlify, GitHub Pages, … — vendor-agnostic)
 * - VMZ --profile stays `static` (delivery contract), not the folder name
 */
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const homepageRoot = join(dirname(fileURLToPath(import.meta.url)), "..");

/** Default filesystem target under dist/ (CDN upload root). */
export const DEFAULT_OUT_TARGET = "cdn";

export function readDefaultProfile() {
    try {
        const raw = readFileSync(join(homepageRoot, "vmz.config.ts"), "utf8");
        const m = raw.match(/default:\s*["']([^"']+)["']/);
        if (m?.[1]) return m[1];
    } catch {
        /* fall through */
    }
    return "static";
}

/** dist/<target> — e.g. dist/cdn */
export function distDirForTarget(target = DEFAULT_OUT_TARGET) {
    return join("dist", target);
}
