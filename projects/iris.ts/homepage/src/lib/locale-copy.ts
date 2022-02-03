import { docsHomePath } from "./site.ts";

export function readLocaleId(preferred?: string | null): string {
    if (preferred === "en-us" || preferred === "zh-hans") return preferred;
    if (typeof document === "undefined") return "zh-hans";
    const html = document.documentElement.getAttribute("lang");
    if (html === "en-us" || html === "en") return "en-us";
    return "zh-hans";
}

export function docsPaths(localeId: string) {
    const root = docsHomePath(localeId);
    return {
        docsRootHref: root,
        guideHref: `${root}guide/getting-started`,
        hostsHref: `${root}guide/hosts`,
    };
}

export function watchDocumentLocale(cb: (id: string) => void): (() => void) | null {
    if (typeof document === "undefined") return null;
    const obs = new MutationObserver(() => {
        cb(readLocaleId());
    });
    obs.observe(document.documentElement, {
        attributes: true,
        attributeFilter: ["lang"],
    });
    return () => obs.disconnect();
}

export function withLocaleHint<T>(localeId: string, fn: () => T): T {
    if (typeof document !== "undefined") {
        document.documentElement.setAttribute("lang", localeId);
    }
    return fn();
}
