/** Shared homepage locale helpers for #locales message refresh after LocaleTransition. */

import { docsHomePath } from "./site.js";

export type LocaleId = "zh-hans" | "en-us" | string;

export function readLocaleId(preferred?: string | null): LocaleId {
    if (preferred === "en-us" || preferred === "zh-hans") return preferred;
    if (typeof document !== "undefined") {
        const fromAttr = document.documentElement.getAttribute("data-locale");
        if (fromAttr === "en-us" || fromAttr === "zh-hans") return fromAttr;
        const lang = document.documentElement.getAttribute("lang");
        if (lang === "en-us" || lang === "en") return "en-us";
    }
    return "zh-hans";
}

export function docsPaths(localeId: LocaleId) {
    const root = docsHomePath(localeId === "en-us" ? "en-us" : "zh-hans");
    return {
        docsRootHref: root,
        guideHref: `${root}guide/getting-started`,
        hostsHref: `${root}guide/hosts`,
    };
}

/** Run fn while message modules resolve preferred LocaleId via __vmzLocaleIdHint. */
export function withLocaleHint<T>(localeId: LocaleId, fn: () => T): T {
    const g = typeof globalThis !== "undefined" ? (globalThis as Record<string, unknown>) : null;
    const prev = g ? g.__vmzLocaleIdHint : undefined;
    if (g) g.__vmzLocaleIdHint = localeId;
    try {
        return fn();
    } finally {
        if (g) {
            if (prev === undefined) delete g.__vmzLocaleIdHint;
            else g.__vmzLocaleIdHint = prev;
        }
    }
}

/** Observe html[data-locale] so LocaleTransition refreshes sibling chrome. */
export function watchDocumentLocale(onChange: (localeId: string) => void): (() => void) | null {
    if (typeof document === "undefined" || typeof MutationObserver === "undefined") {
        return null;
    }
    const el = document.documentElement;
    let last = el.getAttribute("data-locale") || "";
    const obs = new MutationObserver(() => {
        const next = el.getAttribute("data-locale") || "";
        if (next === last) return;
        last = next;
        if (next) onChange(next);
    });
    obs.observe(el, { attributes: true, attributeFilter: ["data-locale"] });
    return () => obs.disconnect();
}
