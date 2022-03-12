#!/usr/bin/env node
/**
 * Post-build for Iris homepage on @vmz/vmz 0.1.12+:
 * - Locale-neutral public URLs for strategy `none` (/d/…, not /d/zh-hans/…)
 * - entry-client duplicate Icon import guard (until upstream dedupe is universal)
 *
 * Document chrome is compiled via DocumentLayout + createRenderHost (no regex template surgery).
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const homepageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const distDir = path.join(homepageRoot, "dist", "cdn");
const DEFAULT_LOCALE = "zh-hans";
const LOCALES = ["zh-hans", "en-us"];

/** @param {{ reason?: string }} [opts] */
export function runPostbuild(opts = {}) {
    if (!fs.existsSync(distDir)) {
        console.warn("postbuild-site: skip — dist/cdn missing");
        return;
    }
    patchEntryClient(path.join(distDir, "entry-client.js"));
    const manifestPath = path.join(distDir, "document.manifest.json");
    if (!fs.existsSync(manifestPath)) {
        console.warn("postbuild-site: no document.manifest.json");
        return;
    }
    const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
    republishLocaleNeutralDocuments(manifest);
    const tag = opts.reason ? ` (${opts.reason})` : "";
    console.log(`postbuild-site: locale-neutral /d republish${tag}`);
}

function main() {
    runPostbuild();
}

/** @param {string} entryPath */
function patchEntryClient(entryPath) {
    if (!fs.existsSync(entryPath)) return;
    let code = fs.readFileSync(entryPath, "utf8");
    const before = code;
    code = code.replace(
        /import Icon from "\.\/components\/Icon\.client\.js[^"]*";\nimport Icon from "\.\/components\/Icon\.client\.js[^"]*";\n/,
        'import Icon from "./components/Icon.client.js";\n',
    );
    code = code.replace(/(\tIcon,\n)\tIcon,/g, "$1");
    if (code !== before) {
        fs.writeFileSync(entryPath, code, "utf8");
        console.log("postbuild-site: deduped Icon in entry-client.js");
    }
}

/** @param {any} manifest */
function republishLocaleNeutralDocuments(manifest) {
    const routeBase = manifest.mounts?.[0]?.routeBase || "/d";
    const base = routeBase.replace(/^\//, "").replace(/\/$/, "");
    const localeStore = path.join(distDir, base, ".vmz", "locales");
    fs.mkdirSync(localeStore, { recursive: true });

    /** @type {Map<string, Array<{ locale: string, htmlPath: string, pageKey: string }>>} */
    const byPublicKey = new Map();

    for (const page of manifest.pages || []) {
        const locale = page.identity?.locale;
        const pageKey = page.identity?.pageKey || "index";
        if (!locale) continue;
        const srcRel = `${base}/${locale}/${pageKey === "index" ? "index.html" : `${pageKey}.html`}`;
        const srcAbs = path.join(distDir, srcRel.replace(/\\/g, "/"));
        if (!fs.existsSync(srcAbs)) continue;

        let html = fs.readFileSync(srcAbs, "utf8");
        html = rewriteDocNavLinks(html);

        const archiveRel = path.posix.join(
            base,
            ".vmz",
            "locales",
            locale,
            pageKey === "index" ? "index.html" : `${pageKey}.html`,
        );
        const archiveAbs = path.join(distDir, archiveRel);
        fs.mkdirSync(path.dirname(archiveAbs), { recursive: true });
        fs.writeFileSync(archiveAbs, html, "utf8");

        const publicKey = pageKey === "index" ? "index" : pageKey.replace(/\\/g, "/");
        const list = byPublicKey.get(publicKey) || [];
        list.push({ locale, htmlPath: archiveRel, pageKey: publicKey });
        byPublicKey.set(publicKey, list);
    }

    for (const [pageKey, variants] of byPublicKey) {
        const def = variants.find((v) => v.locale === DEFAULT_LOCALE) || variants[0];
        const publicRel =
            pageKey === "index" ? path.posix.join(base, "index.html") : path.posix.join(base, `${pageKey}.html`);
        const publicAbs = path.join(distDir, publicRel);
        fs.mkdirSync(path.dirname(publicAbs), { recursive: true });

        let html = fs.readFileSync(path.join(distDir, def.htmlPath), "utf8");
        const altMeta = variants
            .filter((v) => v.locale !== def.locale)
            .map((v) => `${v.locale}:${"/" + v.htmlPath.replace(/\\/g, "/")}`)
            .join(",");
        if (altMeta && !html.includes('name="vmz-locale-variants"')) {
            html = html.replace("</head>", `  <meta name="vmz-locale-variants" content="${altMeta}">\n</head>`);
        }
        if (!html.includes("documentLocaleSwapScript")) {
            html = html.replace("</body>", `${documentLocaleSwapScript()}\n</body>`);
        }
        fs.writeFileSync(publicAbs, html, "utf8");
    }

    for (const loc of LOCALES) {
        const legacy = path.join(distDir, base, loc);
        if (fs.existsSync(legacy)) fs.rmSync(legacy, { recursive: true, force: true });
    }

    writePrettyJson(path.join(distDir, base, ".vmz", "locale-routes.json"), {
        strategy: "none",
        defaultLocale: DEFAULT_LOCALE,
        locales: LOCALES,
        pages: [...byPublicKey.keys()],
    });
}

function documentLocaleSwapScript() {
    return `<script>/* documentLocaleSwapScript */(function(){try{var pref=localStorage.getItem("vmz.locale")||${JSON.stringify(DEFAULT_LOCALE)};var cur=document.documentElement.getAttribute("data-locale")||${JSON.stringify(DEFAULT_LOCALE)};if(pref===cur)return;var meta=document.querySelector('meta[name="vmz-locale-variants"]');if(!meta||!meta.content)return;var map={};meta.content.split(",").forEach(function(pair){var i=pair.indexOf(":");if(i<0)return;map[pair.slice(0,i)]=pair.slice(i+1);});var alt=map[pref];if(!alt)return;fetch(alt).then(function(r){return r.text();}).then(function(raw){var doc=new DOMParser().parseFromString(raw,"text/html");var src=doc.querySelector(".doc-content");var dst=document.querySelector(".doc-content");if(src&&dst)dst.innerHTML=src.innerHTML;var nav=doc.querySelector(".doc-subnav");var navDst=document.querySelector(".doc-subnav");if(nav&&navDst)navDst.innerHTML=nav.innerHTML;var hdr=doc.querySelector('[data-vmz-fixture="site-header"]');var hdrDst=document.querySelector('[data-vmz-fixture="site-header"]');if(hdr&&hdrDst)hdrDst.outerHTML=hdr.outerHTML;var ftr=doc.querySelector('[data-vmz-fixture="site-footer"]');var ftrDst=document.querySelector('[data-vmz-fixture="site-footer"]');if(ftr&&ftrDst)ftrDst.outerHTML=ftr.outerHTML;document.documentElement.setAttribute("data-locale",pref);document.documentElement.setAttribute("lang",pref);window.__vmzLocaleIdHint=pref;document.querySelectorAll("[data-vmz-locale-pick], .locale-switch__btn[data-vmz-locale-pick]").forEach(function(btn){var on=btn.getAttribute("data-vmz-locale-pick")===pref;btn.setAttribute("aria-current",on?"true":"false");btn.classList.toggle("is-active",on);});}).catch(function(){});}catch(e){}})();</script>`;
}

function rewriteDocNavLinks(html) {
    return html
        .replace(/href="\.\/zh-hans\//g, 'href="./')
        .replace(/href="\.\/en-us\//g, 'href="./')
        .replace(/href="\/d\/zh-hans\//g, 'href="/d/')
        .replace(/href="\/d\/en-us\//g, 'href="/d/');
}

/** @param {string} filePath @param {unknown} data */
function writePrettyJson(filePath, data) {
    fs.mkdirSync(path.dirname(filePath), { recursive: true });
    fs.writeFileSync(filePath, `${JSON.stringify(data, null, 2)}\n`, "utf8");
}

main();
